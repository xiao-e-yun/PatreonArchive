use std::{collections::HashMap, ops::Deref, path::PathBuf};

use futures::future::join_all;
use log::error;
use mime_guess::MimeGuess;
use post_archiver::importer::file_meta::UnsyncFileMeta;
use serde_json::json;

use crate::{api::patreon::PatreonClient, patreon::post::Media};

pub async fn download_files(
    files: Vec<(PathBuf, String)>,
    client: &PatreonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = vec![];

    let mut last_folder = PathBuf::new();
    for (path, url) in files {
        // Create folder if it doesn't exist
        let folder = path.parent().unwrap();
        if last_folder != folder {
            last_folder = folder.to_path_buf();
            tokio::fs::create_dir_all(folder).await?;
        }

        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            if let Err(e) = client.download(&url, path.clone()).await {
                error!("Failed to download {} to {}: {}", url, path.display(), e);
            }
        }));
    }

    join_all(tasks).await;
    Ok(())
}

pub trait PatreonFileMeta
where
    Self: Sized,
{
    fn from_url(url: String) -> Self;
    fn from_media(image: Media) -> Self;
    fn from_audio_thumb(image: Media, filename: String) -> Self;
}

pub struct UnsyncFileMetaWithUrl(pub UnsyncFileMeta, pub String);
impl Deref for UnsyncFileMetaWithUrl {
    type Target = UnsyncFileMeta;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PatreonFileMeta for UnsyncFileMetaWithUrl {
    fn from_url(url: String) -> Self {
        let mut filename = url.split('/').next_back().unwrap().to_string();
        filename.truncate(filename.find('?').unwrap_or(url.len()));

        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();
        let extra = Default::default();

        Self(UnsyncFileMeta {
            filename,
            mime,
            extra,
        }, url)
    }
    fn from_media(media: Media) -> Self {
        let filename = media.file_name.unwrap_or_else(|| {
            media
                .download_url
                .split('/')
                .next_back()
                .unwrap()
                .to_string()
        });
        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        let mut extra = HashMap::new();

        let dimensions = &media.metadata.dimensions;
        if let Some(dimensions) = dimensions {
            extra.insert("width".to_string(), json!(dimensions.w));
            extra.insert("height".to_string(), json!(dimensions.h));
        }

        let duration_s = &media.metadata.duration_s;
        if let Some(duration_s) = duration_s {
            extra.insert("duration_s".to_string(), json!(duration_s));
        }

        Self(UnsyncFileMeta {
            filename,
            mime,
            extra,
        }, media.download_url)
    }
    fn from_audio_thumb(media: Media, filename: String) -> Self {
        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        let mut extra = HashMap::new();

        let dimensions = &media.metadata.dimensions;
        if let Some(dimensions) = dimensions {
            extra.insert("width".to_string(), json!(dimensions.w));
            extra.insert("height".to_string(), json!(dimensions.h));
        }

        Self(UnsyncFileMeta {
            filename,
            mime,
            extra,
        }, media.download_url)
    }
}
