use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use futures::future::try_join_all;
use log::warn;
use mime_guess::MimeGuess;
use post_archiver::importer::{UnsyncContent, UnsyncFileMeta};
use post_archiver_utils::Result;
use serde_json::{json, Value};

use crate::{api::PatreonDownloader, patreon::post::Media};

/// Default thumbnails have no meaningful filename in their URL.
const DEFAULT_THUMB_PREFIX: &str = "https://www.patreon.com/media-u/v3/";

/// A file to download, with the metadata it will be imported under.
#[derive(Debug, Clone)]
pub struct PendingFile {
    pub filename: String,
    pub mime: String,
    pub extra: HashMap<String, Value>,
    pub url: String,
}

#[derive(Debug, Clone)]
pub enum PendingContent {
    Text(String),
    File(PendingFile),
}

impl PendingFile {
    pub fn from_url(url: String) -> Self {
        if url.starts_with(DEFAULT_THUMB_PREFIX) {
            return Self::new("thumb.jpg".to_string(), "image/jpeg".to_string(), url);
        };

        let mut filename = url.split('/').next_back().unwrap().to_string();
        if let Some(query) = filename.find('?') {
            filename.truncate(query);
        }

        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        Self::new(filename, mime, url)
    }

    pub fn from_media(media: Media) -> Self {
        let mut filename = media.file_name.clone().unwrap_or_else(|| {
            media
                .download_url
                .split('/')
                .next_back()
                .unwrap()
                .to_string()
        });

        if filename.starts_with(DEFAULT_THUMB_PREFIX) {
            filename = "thumb.jpg".to_string();
        };

        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        let mut extra = media_dimensions(&media);
        if let Some(duration_s) = &media.metadata.duration_s {
            extra.insert("duration_s".to_string(), json!(duration_s));
        }

        Self {
            filename,
            mime,
            extra,
            url: media.download_url,
        }
    }

    pub fn from_audio_thumb(media: Media, filename: String) -> Self {
        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        Self {
            filename,
            mime,
            extra: media_dimensions(&media),
            url: media.download_url,
        }
    }

    fn new(filename: String, mime: String, url: String) -> Self {
        Self {
            filename,
            mime,
            extra: HashMap::new(),
            url,
        }
    }
}

fn media_dimensions(media: &Media) -> HashMap<String, Value> {
    let mut extra = HashMap::new();
    if let Some(dimensions) = &media.metadata.dimensions {
        extra.insert("width".to_string(), json!(dimensions.w));
        extra.insert("height".to_string(), json!(dimensions.h));
    }
    extra
}

/// Download every pending file once into `with_in` (the same URL can appear twice,
/// e.g. as thumbnail and content), then turn pending contents into importable ones.
pub async fn download_contents(
    contents: Vec<PendingContent>,
    thumb: Option<PendingFile>,
    downloader: &PatreonDownloader,
    with_in: &Path,
) -> Result<(Vec<UnsyncContent>, Option<UnsyncFileMeta>)> {
    let mut urls: Vec<&str> = contents
        .iter()
        .filter_map(|c| match c {
            PendingContent::File(f) => Some(f.url.as_str()),
            PendingContent::Text(_) => None,
        })
        .chain(thumb.iter().map(|f| f.url.as_str()))
        .collect();
    urls.sort_unstable();
    urls.dedup();

    let downloaded: HashMap<String, PathBuf> = try_join_all(urls.into_iter().map(|url| async move {
        match downloader.download(with_in, url).await {
            Ok(path) => Ok(Some((url.to_string(), path))),
            // Placeholder images of embed posts often 404; don't fail the whole post for them.
            Err(e) if url.starts_with(DEFAULT_THUMB_PREFIX) => {
                warn!("Skipping placeholder image {url}: {e}");
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }))
    .await?
    .into_iter()
    .flatten()
    .collect();

    let into_meta = |file: PendingFile| {
        let source = downloaded.get(&file.url)?.clone();
        Some(UnsyncFileMeta::new(file.filename, file.mime, source).extra(file.extra))
    };

    let contents = contents
        .into_iter()
        .filter_map(|c| match c {
            PendingContent::Text(text) => Some(UnsyncContent::Text(text)),
            PendingContent::File(file) => into_meta(file).map(UnsyncContent::File),
        })
        .collect();

    Ok((contents, thumb.and_then(into_meta)))
}
