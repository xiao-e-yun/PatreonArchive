mod body;
pub mod file;

use std::collections::HashSet;

use chrono::DateTime;
use file::{download_contents, PendingFile};
use log::{error, info, warn};
use plyne::Output;
use post_archiver::importer::{
    post::UnsyncPost, UnsyncAlias, UnsyncAuthor, UnsyncCollection, UnsyncContent, UnsyncFileMeta,
    UnsyncTag,
};
use serde_json::json;
use tempfile::tempdir;
use tokio::sync::Mutex;

use crate::{
    api::{PatreonClient, PatreonDownloader},
    config::{Config, IncOnDrop, ProgressSet},
    context::Context,
    patreon::{comment::Comment, post::Post},
    Manager,
};

#[derive(Debug)]
pub enum PostEvent {
    /// A post from the campaign listing, with fresh media URLs.
    Listed(Box<Post>),
    /// A post that failed on an earlier run. Media URLs expire, so it is fetched again.
    Retry { campaign: String, id: String },
}

impl PostEvent {
    fn id(&self) -> &str {
        match self {
            PostEvent::Listed(post) => &post.id,
            PostEvent::Retry { id, .. } => id,
        }
    }

    fn campaign(&self) -> &str {
        match self {
            PostEvent::Listed(post) => &post.campaign.id,
            PostEvent::Retry { campaign, .. } => campaign,
        }
    }
}

/// A post that failed to import: (campaign_id, post_id).
type FailedPost = (String, String);

pub async fn posts(
    mut posts_pipeline: Output<PostEvent>,
    client: &PatreonClient,
    config: &Config,
    manager: &Manager,
    context: &Mutex<Context>,
    progress: &ProgressSet,
) {
    let tasks: TaskSet = TaskSet::new("posts").limit(4);
    let downloader = PatreonDownloader::new(config, progress.files.clone());

    // A retried post can also come back in the listing.
    let mut seen = HashSet::new();

    while let Some(event) = posts_pipeline.recv().await {
        let guard = IncOnDrop(progress.posts.clone());
        if !seen.insert(event.id().to_string()) {
            continue;
        }

        tasks
            .spawn(post_task(
                event,
                client.clone(),
                downloader.clone(),
                manager.clone(),
                guard,
            ))
            .await;
    }

    let results = tasks.join_all().await;
    progress.posts.finish();
    progress.files.finish();

    // `creator_posts` has already finished (it owns a pipeline sender), so this can't race it.
    let mut ctx = context.lock().await;
    for (campaign, id) in results.into_iter().filter_map(Result::err) {
        ctx.campaigns.entry(campaign).or_default().failed.insert(id);
    }
}

type TaskSet = post_archiver_utils::TaskSet<(), FailedPost>;

async fn post_task(
    event: PostEvent,
    client: PatreonClient,
    downloader: PatreonDownloader,
    manager: Manager,
    _guard: IncOnDrop,
) -> Result<(), FailedPost> {
    let failed = (event.campaign().to_string(), event.id().to_string());

    let post = match event {
        PostEvent::Listed(post) => *post,
        PostEvent::Retry { id, .. } => match client.get_post(&id).await {
            Ok(post) => post,
            Err(e) => {
                error!("Failed to fetch post {id}: {e}");
                return Err(failed);
            }
        },
    };

    if !post.current_user_can_view {
        // e.g. a retried post after the pledge went down; a later pledge increase rescans it.
        warn!("Post {} is no longer viewable, dropping it", post.url);
        return Ok(());
    }

    let comments = match post.comment_count {
        0 => vec![],
        _ => client.get_comments(&post.id).await.unwrap_or_else(|e| {
            error!("Failed to get comments of post {}: {e}", post.id);
            vec![]
        }),
    };

    let Ok(temp_dir) = tempdir() else {
        error!("Failed to create temporary directory for post {}", post.id);
        return Err(failed);
    };

    let thumb = post.image.as_ref().map(|image| {
        let mut thumb = PendingFile::from_url(image.url.clone());
        thumb.extra.insert("width".to_string(), json!(image.width));
        thumb.extra.insert("height".to_string(), json!(image.height));
        thumb
    });

    let (contents, thumb) =
        match download_contents(post.contents(), thumb, &downloader, temp_dir.path()).await {
            Ok(downloaded) => downloaded,
            Err(e) => {
                error!("Failed to download files of post {}: {e}", post.url);
                return Err(failed);
            }
        };

    let title = post.title.clone();
    let source = post.url.clone();
    let unsync = conversion_post(post, contents, thumb, comments);

    // `import_post` copies files with blocking I/O; keep it off the async runtime.
    // `temp_dir` must outlive the import, since the files are copied from it.
    let result = tokio::task::spawn_blocking(move || {
        let result = manager.blocking_lock().importer().import_post(unsync);
        drop(temp_dir);
        result
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            info!("Post imported: {title}");
            Ok(())
        }
        Ok(Err(e)) => {
            error!("Failed to import post {source}: {e}");
            Err(failed)
        }
        Err(e) => {
            error!("Import task panicked for {source}: {e}");
            Err(failed)
        }
    }
}

fn conversion_post(
    post: Post,
    contents: Vec<UnsyncContent>,
    thumb: Option<UnsyncFileMeta>,
    comments: Vec<Comment>,
) -> UnsyncPost {
    let mut tags = vec![];
    if post.is_free() {
        tags.push(UnsyncTag {
            name: "free".to_string(),
            platform: None,
        });
    }

    let collections = post
        .user_defined_tags
        .iter()
        .map(|tag| {
            UnsyncCollection::new(
                tag.value.clone(),
                format!(
                    "{}/posts?filters[tag]={}",
                    post.campaign.url,
                    urlencoding::encode(&tag.value)
                ),
            )
        })
        .collect();

    let author = UnsyncAuthor::new(post.campaign.name.clone()).aliases(vec![UnsyncAlias::new(
        "patreon".to_string(),
        post.campaign.id.clone(),
    )
    .link(post.campaign.url.clone())]);

    let comments = comments.into_iter().map(|c| c.into()).collect();

    let published = DateTime::parse_from_rfc3339(&post.published_at)
        .map(|t| t.to_utc())
        .ok();

    let mut unsync = UnsyncPost::new("patreon".to_string(), post.url, post.title, contents)
        .authors(vec![author])
        .tags(tags)
        .thumb(thumb)
        .comments(comments)
        .collections(collections);
    if let Some(published) = published {
        unsync = unsync.published(published).updated(published);
    }
    unsync
}
