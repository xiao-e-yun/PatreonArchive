use std::{collections::HashMap, path::PathBuf};

use futures::future::join_all;
use mime_guess::MimeGuess;
use post_archiver::importer::file_meta::{ImportFileMetaMethod, UnsyncFileMeta};
use serde_json::json;

use crate::{api::patreon::PatreonClient, patreon::post::Media};

pub async fn download_files(
    files: Vec<(PathBuf, ImportFileMetaMethod)>,
    client: &PatreonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = vec![];

    let mut last_folder = PathBuf::new();
    for (path, method) in files {
        let ImportFileMetaMethod::Url(url) = method else {
            unimplemented!()
        };

        // Create folder if it doesn't exist
        let folder = path.parent().unwrap();
        if last_folder != folder {
            last_folder = folder.to_path_buf();
            tokio::fs::create_dir_all(folder).await?;
        }

        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client
                .download(&url, path)
                .await
                .expect("Failed to download file");
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
    #[allow(unused)]
    fn from_media(image: Media) -> Self;
}

impl PatreonFileMeta for UnsyncFileMeta {
    fn from_url(url: String) -> Self {
        let mut filename = url.split('/').next_back().unwrap().to_string();
        filename.truncate(filename.find('?').unwrap_or(url.len()));

        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();
        let extra = Default::default();
        let method = ImportFileMetaMethod::Url(url);

        Self {
            filename,
            mime,
            extra,
            method,
        }
    }
    fn from_media(image: Media) -> Self {
        let filename = image.file_name;
        let mime = MimeGuess::from_path(&filename)
            .first_or_octet_stream()
            .to_string();

        let mut extra = HashMap::new();

        let dimensions = &image.metadata.dimensions;
        if let Some(dimensions) = dimensions {
            extra.insert("width".to_string(), json!(dimensions.w));
            extra.insert("height".to_string(), json!(dimensions.h));
        }

        let method = ImportFileMetaMethod::Url(
            image
                .download_url
                .or(image.image_urls.map(|v| v.original))
                .unwrap(),
        );

        Self {
            filename,
            mime,
            extra,
            method,
        }
    }
}
