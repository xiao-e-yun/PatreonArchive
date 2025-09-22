use std::{collections::HashMap, sync::Arc};

use futures::future::try_join_all;
use log::error;
use mime_guess::MimeGuess;
use post_archiver::importer::file_meta::UnsyncFileMeta;
use serde_json::json;
use tokio::{sync::Semaphore, task::JoinSet};

use crate::{api::PatreonClient, patreon::post::Media, Config, FilesPipelineOutput};

pub async fn download_files(config: Config, mut files_pipeline: FilesPipelineOutput) {
    let mut tasks = JoinSet::new();
    let client = PatreonClient::new(&config);

    let semaphore = Arc::new(Semaphore::new(3));
    while let Some((urls, tx)) = files_pipeline.recv().await {
        if urls.is_empty() {
            tx.send(Default::default()).unwrap();
            continue;
        }

        let client = client.clone();
        let semaphore = semaphore.clone();
        tasks.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            match try_join_all(urls.into_iter().map(|url| async {
                let download_path = client.download(&url);
                let result = download_path.await.map(|path| (url, path));
                result.inspect_err(|e| error!("Failed to download file: {e}"))
            }))
            .await
            {
                Ok(urls) => tx.send(urls.into_iter().collect()).unwrap(),
                Err(e) => error!("Failed to receive file URLs: {e}"),
            }
        });
    }

    tasks.join_all().await;
}

pub trait PatreonFileMeta
where
    Self: Sized,
{
    fn from_url(url: String) -> Self;
    fn from_media(image: Media) -> Self;
    fn from_audio_thumb(image: Media, filename: String) -> Self;
}

impl PatreonFileMeta for UnsyncFileMeta<String> {
    fn from_url(url: String) -> Self {
        if url.starts_with("https://www.patreon.com/media-u/v3/") {

            return UnsyncFileMeta::new("thumb.jpg".to_string(), "image/jpeg".to_string(), url)
        };

        let mut filename = url.split('/').next_back().unwrap().to_string();
        filename.truncate(filename.find('?').unwrap_or(url.len()));

        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        UnsyncFileMeta::new(filename, mime, url)
    }
    fn from_media(media: Media) -> Self {
        let mut filename = media.file_name.unwrap_or_else(|| {
            media
                .download_url
                .split('/')
                .next_back()
                .unwrap()
                .to_string()
        });

        if filename.starts_with("https://www.patreon.com/media-u/v3/") {
            filename = "thumb.jpg".to_string();
        };

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

        UnsyncFileMeta {
            filename,
            mime,
            extra,
            data: media.download_url,
        }
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

        UnsyncFileMeta {
            filename,
            mime,
            extra,
            data: media.download_url,
        }
    }
}
