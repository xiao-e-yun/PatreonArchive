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
            audio_file_name = Some(file_name.as_ref().unwrap().rsplit_once('.').unwrap().0);

            let file = UnsyncFileMeta::from_media(audio.clone());
            contents.push(UnsyncContent::File(file));
        }

        for media in filtered_media.into_iter() {
            let thumbnail = media.image_urls.as_ref().map(|e| &e.thumbnail);
            if audio.is_some() && thumbnail == thumb_square_url {
                // the original image of audio cover
                let ext = media
                    .file_name
                    .clone()
                    .unwrap_or_else(|| {
                        media
                            .download_url
                            .split('/')
                            .next_back()
                            .unwrap()
                            .to_string()
                    })
                    .rsplit_once('.')
                    .unwrap()
                    .1
                    .to_string();
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

        if let Some(poll) = self.poll.as_deref() {
            let choices = poll.choices.clone();

            let mut name_width = 4_usize;
            let mut votes_width = 5_usize;
            let mut total_votes = 0;
            for choice in choices.iter() {
                name_width = name_width.max(choice.text_content.len());
                votes_width = votes_width.max(choice.num_responses.checked_ilog10().unwrap_or(0) as usize + 1) + 7; //X (xxx%)
                total_votes += choice.num_responses;
            };
            if total_votes == 0 { total_votes = 1 };

            let mut table = vec![
                format!("| {:^name_width$} | Percentage | {:<votes_width$} |","Name","Votes"),
                format!("|-{}-|------------|-{}-|","-".repeat(name_width),"-".repeat(votes_width)),
            ];

            for choice in choices.iter() {
                let percentage = choice.num_responses as f32 / total_votes as f32;
                let vote = format!("{} ({:.0}%)",choice.num_responses, percentage * 100.);
                table.push(format!("| {:^name_width$} | {:<10} | {:<votes_width$} |", choice.text_content, "#".repeat((percentage * 10.) as usize), vote));
            }

            contents.push(UnsyncContent::Text(table.join("\n")));
        }

        std::mem::take(&mut contents)
    }
}
