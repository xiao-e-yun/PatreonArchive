mod body;
pub mod file;

use std::{
    collections::{hash_map::Entry, HashMap},
    path::PathBuf,
};

use crate::{
    creator::sync_campaign,
    patreon::{comment::Comment, post::Post},
    CampaignPipelineOutput, Client, Config, FilesPipelineInput, Manager, PostsPipelineInput,
    PostsPipelineOutput, Progress, User,
};
use chrono::DateTime;
use file::PatreonFileMeta;
use futures::{future::join_all, try_join};
use log::{error, info, trace};
use post_archiver::{
    importer::{post::UnsyncPost, UnsyncCollection, UnsyncFileMeta, UnsyncTag},
    manager::{PostArchiverConnection, PostArchiverManager},
    AuthorId, PlatformId,
};
use post_archiver_utils::Result;
use serde_json::json;
use tempfile::TempPath;
use tokio::{
    fs::{create_dir_all, File, OpenOptions},
    io,
    sync::oneshot,
};

pub fn filter_posts(
    config: &Config,
    manager: &PostArchiverManager<impl PostArchiverConnection>,
    posts: Vec<Post>,
) -> Vec<Post> {
    posts
        .into_iter()
        .filter(|post| config.filter_post(post))
        .filter(|post| {
            if config.force() {
                return true;
            }

            let updated = DateTime::parse_from_rfc3339(&post.published_at)
                .unwrap()
                .to_utc();
            manager
                .find_post_with_updated(&post.url, &updated)
                .unwrap_or_else(|err| {
                    error!("Failed to check post {}: {}", &post.url, err);
                    None
                })
                .is_none()
        })
        .collect()
}

pub async fn list_posts(
    user: User,
    pb: Progress,
    config: Config,
    client: Client,
    manager: Manager,
    posts_pipeline: PostsPipelineInput,
    files_pipeline: FilesPipelineInput,
    mut campaign_pipeline: CampaignPipelineOutput,
) {
    while let Some(campaign) = campaign_pipeline.recv().await {
        info!("Loading posts of campaign {campaign}");

        let mut next_url = Some(client.get_posts_url(&user, &campaign));
        while let Some(url) = next_url.take() {
            let Ok((posts, next)) = client.get_posts(&url).await else {
                error!("Failed to load posts of campaign {campaign}");
                break;
            };
            next_url = next;

            let posts = filter_posts(&config, &*manager.lock().await, posts);
            pb.posts.inc_length(posts.len() as u64);

            let posts = posts
                .into_iter()
                .map(async |post| {
                    let comments = match post.comment_count {
                        0 => vec![],
                        _ => client.get_comments(&post.id).await.unwrap_or_else(|err| {
                            error!("Failed to get comments of post {}: {}", &post.id, err);
                            vec![]
                        }),
                    };

                    let (tx, rx) = oneshot::channel();

                    let contents = post.files();
                    files_pipeline.send((contents, tx)).unwrap();
                    posts_pipeline.send((post, comments, rx)).unwrap();
                })
                .collect::<Vec<_>>();

            join_all(posts).await;
        }
        pb.creators.inc(1);
    }
    info!(
        "Creators processed: {}/{} creators",
        pb.creators.position(),
        pb.creators.length().unwrap_or_default()
    );
}

pub async fn sync_posts(manager: Manager, mut posts_pipeline: PostsPipelineOutput, pb: Progress) {
    let mut authors = HashMap::new();
    'post: while let Some((post, comments, rx)) = posts_pipeline.recv().await {
        let mut manager = manager.lock().await;

        let platform = manager.import_platform("patreon".to_string()).unwrap();

        let campaign = post.campaign.clone();
        let campaign_id = campaign.id.clone();
        let author = match authors.entry(campaign_id) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => match sync_campaign(&manager, platform, &campaign) {
                Ok(author) => *vacant_entry.insert(author),
                Err(e) => {
                    error!("Failed to sync creator for post: {} {:?}", post.id, e);
                    continue;
                }
            },
        };

        let tx = manager.transaction().unwrap();

        let title = post.title.clone();
        let post = conversion_post(platform, author, post, comments);
        let source = post.source.clone();

        let Ok((_, _, _, files)) = tx.import_post(post, true) else {
            error!("Failed to import post: {source}");
            continue;
        };

        let Ok(mut file_map) = rx.await else {
            error!("Failed to receive file map for post: {source}");
            continue;
        };

        let mut create_dir = true;
        for (path, url) in files {
            if let Err(e) = save_file(&mut file_map, &path, &url, create_dir).await {
                error!("Failed to save file {}: {}", path.display(), e);
                error!("Aborting post import due to file errors: {source}");
                continue 'post;
            };
            create_dir = false;
        }

        tx.commit().unwrap();
        info!("Post imported: {title}");

        pb.posts.inc(1);
    }

    info!(
        "Posts imported: {}/{} posts",
        pb.posts.position(),
        pb.posts.length().unwrap_or_default()
    );

    fn conversion_post(
        platform: PlatformId,
        author: AuthorId,
        post: Post,
        comments: Vec<Comment>,
    ) -> UnsyncPost<String> {
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

        let thumb = post.image.clone().map(|image| {
            let mut meta = if image.url.starts_with("https://www.patreon.com/media-u/v3/") {
                // default thumb url
                UnsyncFileMeta::new("thumb.jpg".to_string(), "image/jpeg".to_string(), image.url)
            } else {
                UnsyncFileMeta::from_url(image.url)
            };
            meta.extra = HashMap::from([
                ("width".to_string(), json!(image.width)),
                ("height".to_string(), json!(image.height)),
            ]);
            meta
        });

        let content = post.contents();

        let comments = comments.into_iter().map(|c| c.into()).collect();

        let published = DateTime::parse_from_rfc3339(&post.published_at)
            .unwrap()
            .to_utc();

        UnsyncPost::new(platform, post.url, post.title, content)
            .published(published)
            .updated(published)
            .authors(vec![author])
            .tags(tags)
            .thumb(thumb)
            .comments(comments)
            .collections(collections)
    }

    async fn save_file(
        file_map: &mut HashMap<String, TempPath>,
        path: &PathBuf,
        url: &str,
        create_dir: bool,
    ) -> Result<()> {
        if create_dir {
            let path = path.parent().unwrap();
            create_dir_all(path).await?;
        }

        let temp = file_map.remove(url).ok_or(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found in map: {url}"),
        ))?;

        let mut open_options = OpenOptions::new();
        let (mut src, mut dst) = try_join!(
            File::open(&temp),
            open_options
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
        )?;

        io::copy(&mut src, &mut dst).await?;
        trace!("File saved: {url} -> {}", path.display());

        Ok(())
    }
}
