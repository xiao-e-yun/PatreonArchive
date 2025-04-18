mod body;
pub mod file;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    api::patreon::PatreonClient,
    config::Config,
    patreon::{comment::Comment, post::Post, User},
};
use chrono::DateTime;
use file::{download_files, PatreonFileMeta};
use log::{debug, error, info, trace};
use post_archiver::{
    importer::{
        file_meta::{ImportFileMetaMethod, UnsyncFileMeta},
        post::UnsyncPost,
    },
    manager::{PostArchiverConnection, PostArchiverManager},
    AuthorId,
};
use rusqlite::Connection;
use serde_json::json;

pub fn filter_unsynced_posts(
    manager: &mut PostArchiverManager<impl PostArchiverConnection>,
    mut posts: Vec<(Post, Vec<Comment>)>,
) -> Result<Vec<(Post, Vec<Comment>)>, rusqlite::Error> {
    posts.retain(|(post, _)| {
        let post_updated = manager
            .check_post_with_updated(
                &post.url,
                &DateTime::parse_from_rfc3339(&post.published_at)
                    .unwrap()
                    .to_utc(),
            )
            .expect("Failed to check post");
        post_updated.is_none()
    });
    Ok(posts)
}

pub async fn get_posts(
    config: &Config,
    user: &User,
    campaign: &str,
) -> Result<Vec<(Post, Vec<Comment>)>, Box<dyn std::error::Error>> {
    let client = PatreonClient::new(config);

    let mut posts = client.get_posts(user, campaign).await?;
    posts.retain(|item| config.filter_post(item));

    const BATCH_SIZE: usize = 10;
    const BATCH_DELAY_MS: u64 = 200;

    let mut comments = Vec::with_capacity(posts.len());

    for chunk in posts.chunks(BATCH_SIZE) {
        let comment_futures = chunk
            .iter()
            .map(|e| client.get_comments(&e.id))
            .collect::<Vec<_>>();

        let batch_comments = futures::future::join_all(comment_futures).await;
        comments.extend(batch_comments);

        tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_DELAY_MS)).await;
    }

    let result = posts
        .into_iter()
        .zip(comments)
        .map(|(post, res)| {
            let comments = res.expect("failed to get comments of post");
            (post, comments)
        })
        .collect();

    Ok(result)
}

pub async fn sync_posts(
    manager: &mut PostArchiverManager<Connection>,
    config: &Config,
    author: AuthorId,
    posts: Vec<(Post, Vec<Comment>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let manager = manager.transaction()?;
    let total_posts = posts.len();

    let mut synced_posts = 0;
    let mut post_files = vec![];
    for (post, comments) in posts {
        info!(" syncing {}", post.title);
        match sync_post(&manager, author, post, comments) {
            Ok(files) => {
                synced_posts += 1;
                info!(" + success");

                if !files.is_empty() {
                    // list all files
                    debug!(" + files:");
                    if log::log_enabled!(log::Level::Debug) {
                        for (file, method) in &files {
                            debug!("    + {}", file.display());
                            trace!("      + {}", method);
                        }
                    }

                    post_files.extend(files);
                }
            }
            Err(e) => error!(" + failed: {}", e),
        }
    }

    let client = PatreonClient::new(config);
    download_files(post_files, &client).await?;

    manager.commit()?;

    info!("{} total", total_posts);
    info!("{} success", synced_posts);
    info!("{} failed", total_posts - synced_posts);

    fn sync_post(
        manager: &PostArchiverManager<impl PostArchiverConnection>,
        author: AuthorId,
        post: Post,
        comments: Vec<Comment>,
    ) -> Result<Vec<(PathBuf, ImportFileMetaMethod)>, Box<dyn std::error::Error>> {
        let mut tags = vec!["patreon".to_string()];
        if post.required_cents() == 0 {
            tags.push("free".to_string());
        }

        let thumb = post.image.clone().map(|image| {
            let mut meta = UnsyncFileMeta::from_url(image.url);
            meta.extra = HashMap::from([
                ("width".to_string(), json!(1200)),
                ("height".to_string(), json!(630)),
            ]);
            meta
        });

        let content = post.content();

        let comments = comments.into_iter().map(|c| c.into()).collect();

        let published = DateTime::parse_from_rfc3339(&post.published_at)
            .unwrap()
            .to_utc();
        let post = UnsyncPost::new(author)
            .source(Some(post.url))
            .published(published)
            .updated(published)
            .tags(tags)
            .title(post.title)
            .content(content)
            .thumb(thumb)
            .comments(comments);

        let (_, files) = post.sync(manager)?;

        Ok(files)
    }

    Ok(())
}
