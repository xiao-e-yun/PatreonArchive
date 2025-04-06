mod body;
pub mod file;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    api::fanbox::FanboxClient,
    config::Config,
    fanbox::{Post, PostListItem},
};
use file::{download_files, FanboxFileMeta};
use log::{debug, error, info};
use post_archiver::{
    importer::{
        file_meta::{ImportFileMetaMethod, UnsyncFileMeta},
        post::UnsyncPost,
    }, manager::{PostArchiverConnection, PostArchiverManager}, AuthorId
};
use rusqlite::Connection;
use serde_json::json;

pub async fn get_post_urls(
    config: &Config,
    creator_id: &str,
) -> Result<Vec<PostListItem>, Box<dyn std::error::Error>> {
    let client = FanboxClient::new(config);
    let mut items = client.get_posts(creator_id).await?;
    items.retain(|item| config.filter_post(item));
    Ok(items)
}

pub fn filter_unsynced_posts(
    manager: &mut PostArchiverManager<impl PostArchiverConnection>,
    mut posts: Vec<PostListItem>,
) -> Result<Vec<PostListItem>, rusqlite::Error> {
    posts.retain(|post| {
        let source = get_source_link(&post.creator_id, &post.id);
        let post_updated = manager
            .check_post_with_updated(&source, &post.updated_datetime)
            .expect("Failed to check post");
        post_updated.is_none()
    });
    Ok(posts)
}

pub async fn get_posts(
    config: &Config,
    posts: Vec<PostListItem>,
) -> Result<Vec<Post>, Box<dyn std::error::Error>> {
    let client = FanboxClient::new(config);
    let mut tasks = vec![];
    for post in posts {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client.get_post(post.id).await.expect("Failed to get post")
        }));
    }

    let mut posts = Vec::new();

    for task in tasks {
        posts.push(task.await?);
    }

    Ok(posts)
}

pub async fn sync_posts(
    manager: &mut PostArchiverManager<Connection>,
    config: &Config,
    author: AuthorId,
    posts: Vec<Post>,
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = manager.transaction()?;
    let total_posts = posts.len();

    let mut synced_posts = 0;
    let mut post_files = vec![];
    for post in posts {
        info!(" syncing {}", post.title);
        match sync_post(&manager, author, post) {
            Ok(files) => {
                synced_posts += 1;
                info!(" + success");

                if !files.is_empty() {
                    // list all files
                    debug!(" + files:");
                    if log::log_enabled!(log::Level::Debug) {
                        for (file, method) in &files {
                            debug!("    + {}", file.display());
                            debug!("      + {}", method);
                        }
                    }

                    post_files.extend(files);
                }
            }
            Err(e) => error!(" + failed: {}", e),
        }
    }

    let client = FanboxClient::new(config);
    download_files(post_files, &client).await?;

    manager.commit()?;

    info!("{} total", total_posts);
    info!("{} success", synced_posts);
    info!("{} failed", total_posts - synced_posts);

    fn sync_post(
        manager: &PostArchiverManager<impl PostArchiverConnection>,
        author: AuthorId,
        post: Post,
    ) -> Result<Vec<(PathBuf, ImportFileMetaMethod)>, Box<dyn std::error::Error>> {
        let source = get_source_link(&post.creator_id, &post.id);

        let mut tags = vec!["fanbox".to_string()];
        if post.fee_required == 0 {
            tags.push("free".to_string());
        }
        if post.has_adult_content {
            tags.push("r-18".to_string());
        }

        let thumb = post.cover_image_url.clone().map(|url| {
            let mut meta = UnsyncFileMeta::from_url(url);
            meta.extra = HashMap::from([
                ("width".to_string(), json!(1200)),
                ("height".to_string(), json!(630)),
            ]);
            meta
        });

        let content = post.body.content();

        let post = UnsyncPost::new(author)
            .source(Some(source))
            .published(post.published_datetime)
            .updated(post.updated_datetime)
            .tags(tags)
            .title(post.title)
            .content(content)
            .thumb(thumb);

        let (_, files) = post.sync(manager)?;

        Ok(files)
    }

    Ok(())
}

pub fn get_source_link(creator_id: &str, post_id: &str) -> String {
    format!("https://{}.fanbox.cc/posts/{}", creator_id, post_id)
}
