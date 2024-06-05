use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    mem,
    path::PathBuf,
    sync::Arc,
};

use chrono::{DateTime, Local};
use log::info;
use mime_guess::MimeGuess;
use post_archiver::{ArchiveComment, ArchiveContent};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use url::Url;

use crate::{
    api::{ArchiveClient, FanboxClient},
    author::Author,
    config::Config,
    utils::{PostType, User},
};

pub type PostListCache = HashMap<u32, DateTime<Local>>;
pub async fn get_post_list(
    authors: Vec<Author>,
    config: &Config,
) -> Result<Vec<u32>, Box<dyn Error>> {
    const CACHE_FILE: &'static str = "postList.json";

    let client = FanboxClient::new(config.clone());

    let mut result = Vec::new();
    let mut awaits = tokio::task::JoinSet::new();

    let cache: Option<Arc<PostListCache>> = config.load_cache(&CACHE_FILE).map(|c| Arc::new(c));

    let cache = {
        info!("Checking posts");
        for author in authors.into_iter() {
            let client = client.clone();
            let cache = cache.clone();
            awaits.spawn(async move { client.get_post_list(author, cache).await });
        }
        cache
    };

    let mut temp_cache = HashMap::new();
    while let Some(res) = awaits.join_next().await {
        let (list, cache) = res?;
        temp_cache.extend(cache);
        result.extend(list);
    }

    {
        let mut cache = cache.unwrap_or_default();
        let cache = Arc::get_mut(&mut cache).unwrap();
        cache.extend(temp_cache);

        config.save_cache(&CACHE_FILE, &cache);
    }

    Ok(result)
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostList {
    pub items: Vec<PostListItem>,
    pub next_url: Option<Url>,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostListItem {
    #[serde_as(as = "DisplayFromStr")]
    pub id: u32,
    pub title: String,
    pub fee_required: u32,
    pub published_datetime: DateTime<Local>,
    pub updated_datetime: DateTime<Local>,
    pub tags: Vec<String>,
    pub is_liked: bool,
    pub like_count: u32,
    pub comment_count: u32,
    pub is_restricted: bool,
    pub user: User,
    pub creator_id: String,
    pub has_adult_content: bool,
    pub cover: Option<Cover>,
    pub excerpt: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Cover {
    CoverImage { url: Url },
    PostImage { url: Url },
}

//===================================================
// Post
//===================================================

pub async fn get_posts(posts: Vec<u32>, config: &Config) -> Result<Vec<Post>, Box<dyn Error>> {
    let mut result = Vec::new();
    let mut awaits = tokio::task::JoinSet::new();

    serde_json::to_string(&posts).unwrap();

    info!("Checking posts");
    let client = FanboxClient::new(config.clone());
    for post in posts {
        let client = client.clone();
        awaits.spawn(async move { client.get_post(post).await });
    }

    while let Some(res) = awaits.join_next().await {
        result.push(res?);
    }

    Ok(result)
}

//===================================================
// Type
//===================================================
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    #[serde_as(as = "DisplayFromStr")]
    id: u32,
    title: String,
    fee_required: u32,
    published_datetime: DateTime<Local>,
    updated_datetime: DateTime<Local>,
    tags: Vec<String>,
    is_liked: bool,
    like_count: u32,
    comment_count: u32,
    is_restricted: bool,
    user: User,
    creator_id: String,
    has_adult_content: bool,
    #[serde(rename = "type")]
    ty: PostType,
    cover_image_url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<PostBody>,
    excerpt: String,
    comment_list: CommentList,
    next_post: Option<PostShort>,
    prev_post: Option<PostShort>,
    image_for_share: Url,
}

impl Post {
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    pub fn author(&self) -> String {
        self.creator_id.clone()
    }
    pub fn title(&self) -> String {
        self.title.clone()
    }
    pub fn published(&self) -> DateTime<Local> {
        self.published_datetime.clone()
    }
    pub fn updated(&self) -> DateTime<Local> {
        self.updated_datetime.clone()
    }
    pub fn comments(&self) -> Vec<Comment> {
        self.comment_list.items.clone()
    }
    pub fn body(&self) -> PostBody {
        self.body
            .clone()
            .expect(format!("Post {} has no body", self.id()).as_str())
    }
}

//===================================================
// Utils
//===================================================

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase", tag = "type")]
pub struct PostBody {
    text: Option<String>,
    blocks: Option<Vec<PostBlock>>,
    images: Option<Vec<PostImage>>,
    videos: Option<Vec<PostVideo>>,
    files: Option<Vec<PostFile>>,
    image_map: Option<BTreeMap<String, PostImage>>,
    file_map: Option<BTreeMap<String, PostFile>>,

    embed_map: Option<BTreeMap<String, PostEmbed>>,
    url_embed_map: Option<BTreeMap<String, PostUrlEmbed>>,
}

impl PostBody {
    pub fn images(&self) -> Vec<PostImage> {
        let mut images = vec![];

        if let Some(list) = self.images.clone() {
            images.extend(list);
        }

        if let Some(map) = self.image_map.clone() {
            images.extend(map.into_values());
        };

        images
    }
    pub fn files(&self) -> Vec<PostFile> {
        let mut files = vec![];

        if let Some(list) = self.files.clone() {
            files.extend(list);
        }

        if let Some(map) = self.file_map.clone() {
            files.extend(map.into_values());
        };

        files
    }
    pub fn videos_files(mut files: Vec<PostFile>) -> (Vec<PostFile>, Vec<PostFile>) {
        let mut videos = vec![];
        for file in mem::take(&mut files) {
            let ext = MimeGuess::from_ext(&file.extension);
            match ext.first_or_text_plain().type_() {
                mime::VIDEO => videos.push(file),
                _ => files.push(file),
            }
        }

        (videos, files)
    }
    pub fn web_videos(&self) -> Vec<String> {
        let videos = self.videos.clone().unwrap_or_default();
        videos.iter().map(|video| Self::map_video(video)).collect()
    }

    pub fn content(&self, path: PathBuf) -> Vec<ArchiveContent> {
        let mut content = vec![];
        content.extend(self.text());

        for image in self.images.clone().unwrap_or_default() {
            let path = path.join(image.filename());
            content.push(ArchiveContent::Image(path.to_string_lossy().to_string()));
        }

        for video in self.videos.clone().unwrap_or_default() {
            content.push(ArchiveContent::Text(Self::map_video(&video)));
        }

        for file in self.files.clone().unwrap_or_default() {
            let path = path.join(file.filename());
            content.push(ArchiveContent::File(path.to_string_lossy().to_string()));
        }

        content
    }

    pub fn text(&self) -> Vec<ArchiveContent> {
        let mut body = vec![];
        if let Some(text) = self.text.clone()  {
            if !text.is_empty() {
                body.push(ArchiveContent::Text(text));
            }
        }

        let blocks = self.blocks.clone().unwrap_or_default();
        for block in blocks {
            body.push(match block {
                PostBlock::P { text, styles } => {
                    if text.is_empty() {
                        ArchiveContent::Text("  ".to_string())
                    } else {
                        ArchiveContent::Text(set_style(text, styles.unwrap_or_default()))
                    }
                }
                PostBlock::Header { text, styles } => ArchiveContent::Text(format!(
                    "# {}",
                    set_style(text, styles.unwrap_or_default())
                )),
                PostBlock::Image { image_id } => {
                    let image = self.image_map.as_ref().unwrap().get(&image_id).unwrap();
                    let url = format!("{}.{}", image.id, image.extension);
                    ArchiveContent::Image(url)
                }
                PostBlock::File { file_id } => {
                    let file = self.file_map.as_ref().unwrap().get(&file_id).unwrap();
                    ArchiveContent::File(file.filename())
                }
                PostBlock::Embed { embed_id } => {
                    let embed = self.embed_map.as_ref().unwrap().get(&embed_id).unwrap();
                    ArchiveContent::Text(Self::map_embed(embed))
                }
                PostBlock::Video { video_id } => {
                    let video = self
                        .videos
                        .as_ref()
                        .unwrap()
                        .iter()
                        .find(|v| v.video_id == video_id)
                        .unwrap();
                    ArchiveContent::Text(Self::map_video(video))
                }
                PostBlock::UrlEmbed { url_embed_id } => {
                    ArchiveContent::Text(format!("> {}", url_embed_id))
                }
            });

            fn set_style(mut text: String, mut styles: Vec<PostBlockStyle>) -> String {
                while let Some(style) = styles.pop() {
                    let offset = style.offset as usize;
                    let length = style.length as usize;
                    let [left, styled, right]: [String; 3] = {
                        let mut left = String::new();
                        let mut styled = String::new();
                        let mut right = String::new();
                        for (index, char) in text.char_indices() {
                            if index < offset {
                                left.push(char);
                            } else if index < offset + length {
                                styled.push(char);
                            } else {
                                right.push(char);
                            }
                        }
                        [left, styled, right]
                    };
                    let styled: String = match style.ty.as_str() {
                        "bold" => format!("**{}**", styled),
                        _ => {
                            println!("Unknown style: {:?}", style);
                            panic!();
                        }
                    };
                    text = format!("{}{}{}", left, styled, right);
                }
                text
            }
        }

        body
    }

    fn map_video(video: &PostVideo) -> String {
        match video.service_provider.as_str() {
            "youtube" => {
                format!("[![youtube](https://img.youtube.com/vi/{}/0.jpg)](https://www.youtube.com/watch?v={})",video.video_id, video.video_id)
            }
            _ => todo!(),
        }
    }

    fn map_embed(embed: &PostEmbed) -> String {
        match embed.service_provider.as_str() {
            "youtube" => {
                format!("[![youtube](https://img.youtube.com/vi/{}/0.jpg)](https://www.youtube.com/watch?v={})",embed.id, embed.id)
            }
            _ => todo!(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "snake_case", tag = "type")]

pub enum PostBlock {
    P {
        text: String,
        styles: Option<Vec<PostBlockStyle>>,
    },
    Header {
        text: String,
        styles: Option<Vec<PostBlockStyle>>,
    },
    Image {
        #[serde(rename = "imageId")]
        image_id: String,
    },
    File {
        #[serde(rename = "fileId")]
        file_id: String,
    },
    Embed {
        #[serde(rename = "embedId")]
        embed_id: String,
    },
    UrlEmbed {
        #[serde(rename = "urlEmbedId")]
        url_embed_id: String,
    },
    Video {
        #[serde(rename = "videoId")]
        video_id: String,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostBlockStyle {
    #[serde(rename = "type")]
    ty: String,
    offset: u32,
    length: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostImage {
    pub id: String,
    pub extension: String,
    pub width: u32,
    pub height: u32,
    pub original_url: Url,
    pub thumbnail_url: Url,
}

impl PostImage {
    pub fn id(&self) -> String {
        self.id.clone()
    }
    pub fn filename(&self) -> String {
        format!("{}.{}", self.id, self.extension)
    }
    pub fn extension(&self) -> String {
        self.extension.clone()
    }
    pub fn url(&self) -> Url {
        self.original_url.clone()
    }
}

impl Into<PostFile> for PostImage {
    fn into(self) -> PostFile {
        PostFile {
            size: 0,
            id: self.id(),
            name: format!("{}.{}", self.id, self.extension),
            url: self.url(),
            extension: self.extension,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostVideo {
    service_provider: String,
    video_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostFile {
    id: String,
    name: String,
    extension: String,
    size: u64,
    url: Url,
}

impl PostFile {
    pub fn id(&self) -> String {
        self.id.clone()
    }
    pub fn filename(&self) -> String {
        self.name.clone()
    }
    pub fn extension(&self) -> String {
        self.extension.clone()
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn url(&self) -> Url {
        self.url.clone()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostEmbed {
    id: String,
    service_provider: String,
    content_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostUrlEmbed {
    id: String,
    html: String,
    from: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct CommentList {
    items: Vec<Comment>,
    next_url: Option<String>,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde_as(as = "DisplayFromStr")]
    id: u32,
    #[serde_as(as = "DisplayFromStr")]
    parent_comment_id: u32,
    #[serde_as(as = "DisplayFromStr")]
    root_comment_id: u32,
    body: String,
    created_datetime: DateTime<Local>,
    like_count: u32,
    is_liked: bool,
    is_own: bool,
    user: User,
    /// Only for root comment
    replies: Option<Vec<Comment>>,
}

impl Comment {
    pub fn user(&self) -> String {
        self.user.name.clone()
    }
    pub fn text(&self) -> String {
        self.body.clone()
    }
    pub fn replies(&self) -> Vec<Comment> {
        self.replies.clone().unwrap_or_default()
    }
}

impl Into<ArchiveComment> for Comment {
    fn into(self) -> ArchiveComment {
        ArchiveComment {
            user: self.user(),
            text: self.body,
            replies: self
                .replies
                .unwrap_or_default()
                .into_iter()
                .map(|reply| reply.into())
                .collect(),
        }
    }
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct PostShort {
    #[serde_as(as = "DisplayFromStr")]
    id: u32,
    title: String,
    published_datetime: DateTime<Local>,
}
