use htmd::{Element, HtmlToMarkdown};
use post_archiver::importer::{UnsyncContent, UnsyncFileMeta};

use crate::{patreon::post::Post, post::file::PatreonFileMeta};

impl Post {
    pub fn content(&self) -> Vec<UnsyncContent> {
        let img_handler = move |_: Element| -> Option<String> { None }; // skip images

        let htmd_converter = HtmlToMarkdown::builder()
            .add_handler(vec!["img"], img_handler)
            .build();

        let markdown = self.content.as_ref().map(|e| {
            UnsyncContent::Text(
                htmd_converter
                    .convert(e)
                    .expect("Failed to convert HTML to markdown")
                    .replace('\n', "<br>"),
            )
        });

        let mut contents = Vec::new();

        if let Some(content) = markdown {
            contents.push(content);
        }

        let audio_id = self.audio.as_deref().map(|e| &e.id);
        let audio_preview_id = self.audio_preview.as_deref().map(|e| &e.id);

        let thumb_square_url = self.image.as_ref().map(|e| &e.thumb_square_url);

        let filtered_media = self
            .media
            .iter()
            .filter(|media| {
                audio_id.is_none_or(|id| &media.id != id)
                    && audio_preview_id.is_none_or(|id| &media.id != id)
            })
            .map(|e| e.as_ref().clone())
            .collect::<Vec<_>>(); // filter audio & audio_preview
        let mut contents = Vec::new();

        let audio = self.audio.as_deref();
        let mut audio_file_name: Option<&str> = None;

        if let Some(audio) = audio {
            let file_name = &audio.file_name;
            audio_file_name = Some(file_name.rsplit_once('.').unwrap().0);

            let file = UnsyncFileMeta::from_media(audio.clone());
            contents.push(UnsyncContent::File(file));
        }

        for media in filtered_media.into_iter() {
            let thumbnail = media.image_urls.as_ref().map(|e| &e.thumbnail);
            if audio.is_some() && thumbnail == thumb_square_url {
                // the original image of audio cover
                let ext = media.file_name.rsplit_once('.').unwrap().1.to_string();
                let file = UnsyncFileMeta::from_audio_thumb(
                    media,
                    format!("{}.thumb.{}", audio_file_name.unwrap(), ext),
                );
                contents.push(UnsyncContent::File(file));
            } else {
                let file = UnsyncFileMeta::from_media(media);
                contents.push(UnsyncContent::File(file));
            }
        }

        std::mem::take(&mut contents)
    }
}
