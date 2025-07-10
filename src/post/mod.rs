mod body;
pub mod file;

use std::collections::HashMap;

use crate::{
    api::patreon::PatreonClient,
    config::Config,
    patreon::{comment::Comment, post::Post, User},
};
use chrono::DateTime;
use file::{download_files, PatreonFileMeta, UnsyncFileMetaWithUrl};
use log::info;
use post_archiver::{
    importer::{post::UnsyncPost, UnsyncTag},
    manager::{PostArchiverConnection, PostArchiverManager},
    AuthorId, PlatformId,
};
use rusqlite::Connection;
use serde_json::json;

pub fn filter_unsynced_posts(
    manager: &PostArchiverManager<impl PostArchiverConnection>,
    mut posts: Vec<(Post, Vec<Comment>)>,
) -> Result<Vec<(Post, Vec<Comment>)>, rusqlite::Error> {
    posts.retain(|(post, _)| {
        let post_updated = manager
            .find_post_with_updated(
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

    let platform = manager.import_platform("patreon".to_string())?;

    let posts = posts
        .into_iter()
        .map(|(post, comments)| conversion_post(platform, author, post, comments))
        .collect::<Result<Vec<_>, _>>()?;

    let (_posts, post_files) = manager.import_posts(posts, true)?;

    let client = PatreonClient::new(config);
    download_files(post_files, &client).await?;

    manager.commit()?;

    info!("{total_posts} total");

    fn conversion_post(
        platform: PlatformId,
        author: AuthorId,
        post: Post,
        comments: Vec<Comment>,
    ) -> Result<(UnsyncPost, HashMap<String, String>), Box<rusqlite::Error>> {
        let mut tags = vec![];
        if post.is_free() {
            tags.push(UnsyncTag {
                name: "free".to_string(),
                platform: None,
            });
        }

        let thumb = post.image.clone().map(|image| {
            let mut meta = UnsyncFileMetaWithUrl::from_url(image.url);
            meta.0.extra = HashMap::from([
                ("width".to_string(), json!(1200)),
                ("height".to_string(), json!(630)),
            ]);
            meta
        });

        let (content, mut files) = post.content_with_files();

        if let Some(thumb) = &thumb {
            files.insert(thumb.0.filename.clone(), thumb.1.clone());
        }

        let comments = comments.into_iter().map(|c| c.into()).collect();

        let published = DateTime::parse_from_rfc3339(&post.published_at)
            .unwrap()
            .to_utc();
        let post = UnsyncPost::new(platform, post.url, post.title, content)
            .published(published)
            .updated(published)
            .authors(vec![author])
            .tags(tags)
            .thumb(thumb.map(|e| e.0))
            .comments(comments);

        Ok((post, files))
    }

    Ok(())
}
