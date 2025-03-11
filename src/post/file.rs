use std::path::PathBuf;

use futures::future::join_all;
use log::info;
use mime_guess::MimeGuess;
use post_archiver::{AuthorId, FileMetaId, PostId};
use rusqlite::{params, Transaction};
use serde_json::{json, Value};

use crate::{
    api::fanbox::FanboxClient,
    fanbox::{PostBody, PostFile, PostImage},
};

pub fn get_files(
    cover_url: Option<&String>,
    post_body: &PostBody,
    author: AuthorId,
    post: PostId,
) -> Vec<PostFileMeta> {
    let mut files = post_body.files(AuthorId::from(author), PostId::from(post));
    if let Some(cover_url) = cover_url {
        let mut cover = PostFileMeta::from_url(cover_url.clone(), author, post);
        cover.extra = json!({
            "width": 1200,
            "height": 630,
        });
        files.push(cover);
    }
    files
}

pub fn sync_files(
    tx: &mut Transaction,
    files: Vec<PostFileMeta>,
) -> Result<Vec<SyncedFile>, Box<dyn std::error::Error>> {
    let mut insert_file_stmt = tx.prepare_cached(
        "INSERT INTO file_metas (filename,author,post,mime,extra) VALUES (?,?,?,?,?) RETURNING id",
    )?;

    let mut collect = vec![];
    for file in files {
        let id: FileMetaId = insert_file_stmt
            .query_row(
                params![
                    &file.filename,
                    file.author,
                    file.post,
                    &file.mime,
                    serde_json::to_string(&file.extra).unwrap(),
                ],
                |row| row.get(0),
            )
            .unwrap();

        let path = PathBuf::from(file.author.to_string())
            .join(file.post.to_string())
            .join(&file.filename);
        let url = file.url.clone();
        collect.push(SyncedFile {
            id,
            path,
            url,
            raw_id: file.id,
        });
    }
    Ok(collect)
}

pub async fn download_files(
    files: Vec<SyncedFile>,
    client: FanboxClient,
    output: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = vec![];

    let mut last_folder = PathBuf::new();
    for file in files {
        let path = output.join(&file.path);

        if !client.overwrite() && path.exists() {
            info!("Download was skip ({})", path.display());
            continue;
        }

        // Create folder if it doesn't exist
        let folder = path.parent().unwrap();
        if last_folder != folder {
            last_folder = folder.to_path_buf();
            tokio::fs::create_dir_all(folder).await?;
        }

        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client
                .download(&file.url, path)
                .await
                .expect("Failed to download file");
        }));
    }

    join_all(tasks).await;
    Ok(())
}

#[derive(Debug)]
pub struct PostFileMeta {
    pub id: String,
    pub filename: String,
    pub author: AuthorId,
    pub post: PostId,
    pub url: String,
    pub mime: String,
    pub extra: Value,
}

impl PostFileMeta {
    pub fn from_url(url: String, author: AuthorId, post: PostId) -> Self {
        let id = url.clone();
        let filename = url.split('/').last().unwrap().to_string();
        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();
        let extra = Default::default();

        Self {
            id,
            filename,
            author,
            post,
            url,
            mime,
            extra,
        }
    }
    pub fn from_image(image: PostImage, author: AuthorId, post: PostId) -> Self {
        let id = image.id();
        let filename = image.filename();
        let url = image.url();
        let mime = image.mime();
        let extra = json!({
            "width": image.width,
            "height": image.height,
        });

        Self {
            id,
            filename,
            author,
            post,
            url,
            mime,
            extra,
        }
    }
    pub fn from_file(file: PostFile, author: AuthorId, post: PostId) -> Self {
        let id = file.id();
        let filename = file.filename();
        let url = file.url();
        let mime = file.mime();

        Self {
            id,
            filename,
            author,
            post,
            url,
            mime,
            extra: Default::default(),
        }
    }
}

impl PostBody {
    pub fn files(&self, author: AuthorId, post: PostId) -> Vec<PostFileMeta> {
        let mut files: Vec<PostFileMeta> = vec![];

        if let Some(list) = self.images.clone() {
            files.extend(post_images_to_files(list, author, post));
        }

        if let Some(map) = self.image_map.clone() {
            files.extend(post_images_to_files(
                map.into_values().collect(),
                author,
                post,
            ));
        };

        if let Some(list) = self.files.clone() {
            files.extend(psot_files_to_files(list, author, post));
        }

        if let Some(map) = self.file_map.clone() {
            files.extend(psot_files_to_files(
                map.into_values().collect(),
                author,
                post,
            ));
        };

        // util function
        fn post_images_to_files(
            images: Vec<PostImage>,
            author: AuthorId,
            post: PostId,
        ) -> Vec<PostFileMeta> {
            images
                .into_iter()
                .map(|image| PostFileMeta::from_image(image, author, post))
                .collect()
        }

        fn psot_files_to_files(
            files: Vec<PostFile>,
            author: AuthorId,
            post: PostId,
        ) -> Vec<PostFileMeta> {
            files
                .into_iter()
                .map(|file| PostFileMeta::from_file(file, author, post))
                .collect()
        }

        files
    }
}

#[derive(Debug)]
pub struct SyncedFile {
    pub path: PathBuf,
    pub url: String,
    pub raw_id: String,
    pub id: FileMetaId,
}
