use std::{collections::{HashMap, HashSet}, hash::Hash, path::PathBuf};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

//==============================================================================
// List
//==============================================================================
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct ArchiveAuthorsList(Vec<ArchiveAuthorsItem>);

impl ArchiveAuthorsList {
    pub fn from_vector(vec: Vec<ArchiveAuthor>) -> Self {
        let mut vec: Vec<ArchiveAuthorsItem> = vec.into_iter().map(|a| a.into()).collect();
        vec.sort_by(|a, b| a.id.cmp(&b.id));
        ArchiveAuthorsList(vec)
    }
    pub fn extend(&mut self, rhs: Self) {
        let mut authors_map = HashMap::new();

        for author in self.0.iter().cloned() {
            authors_map.insert(author.id.clone(), author);
        }

        for author in rhs.0.iter().cloned() {
            if let Some(old_author) = authors_map.get_mut(&author.id) {
                old_author.extend(author);
            } else {
                authors_map.insert(author.id.clone(), author);
            }
        }

        let mut authors: Vec<ArchiveAuthorsItem> = authors_map.into_values().collect();
        authors.sort_by(|a, b| a.id.cmp(&b.id));
        self.0 = authors;
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash, PartialEq, Eq)]
pub struct ArchiveAuthorsItem {
    pub id: String,
    pub name: String,
    pub r#type: ArchiveByType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<ArchiveFile>,
}

impl ArchiveAuthorsItem {
    pub fn extend(&mut self, rhs: Self) {
        self.id = rhs.id;
        self.name = rhs.name;
        self.r#type = rhs.r#type;
        self.thumb = rhs.thumb.or(self.thumb.clone());
    }
}

//==============================================================================
// Author
//==============================================================================
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveAuthor {
    pub id: String,
    pub name: String,
    pub posts: Vec<String>,
    pub r#type: ArchiveByType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<ArchiveFile>,
}

impl ArchiveAuthor {
    pub fn extend(&mut self, rhs: Self) {
        let mut posts = HashSet::new();
        posts.extend(self.posts.iter().cloned());
        posts.extend(rhs.posts.iter().cloned());
        let mut posts: Vec<String> = posts.into_iter().collect();
        posts.sort();
        posts.reverse();

        self.id = rhs.id;
        self.posts = posts;
        self.name = rhs.name;
        self.r#type = rhs.r#type;
        self.thumb = rhs.thumb.or(self.thumb.clone());
    }
}

impl Into<ArchiveAuthorsItem> for ArchiveAuthor {
    fn into(self) -> ArchiveAuthorsItem {
        ArchiveAuthorsItem {
            id: self.id,
            name: self.name,
            r#type: self.r#type,
            thumb: self.thumb,
        }
    }
}
//==============================================================================
// Post
//==============================================================================
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePost {
    pub id: String,
    pub author: String,
    pub r#type: ArchiveByType,
    pub files: Vec<ArchiveFile>,
    pub updated: DateTime<Local>,
    pub published: DateTime<Local>,
    pub thumb: Option<ArchiveFile>,
    pub content: Vec<ArchiveContent>,
    pub comments: Vec<ArchiveComment>,
}

impl Into<ArchivePostShort> for ArchivePost {
    fn into(self) -> ArchivePostShort {
        ArchivePostShort {
            id: self.id,
            author: self.author,
            r#type: self.r#type,
            updated: self.updated,
            thumb: self.thumb,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePostShort {
    pub id: String,
    pub author: String,
    pub r#type: ArchiveByType,
    pub updated: DateTime<Local>,
    pub thumb: Option<ArchiveFile>,
}

//==============================================================================
// Utils
//==============================================================================

#[derive(Deserialize, Serialize, Debug, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase",tag = "type")]
pub enum ArchiveFile {
    Image {
        width: u32,
        height: u32,
        filename: PathBuf,
        path: PathBuf,
    },
    Video {
        filename: PathBuf,
        path: PathBuf,
    },
    File {
        filename: PathBuf,
        path: PathBuf,
    },
}
impl ArchiveFile {
    pub fn filename(&self) -> &PathBuf {
        match self {
            ArchiveFile::Image { filename, .. } => filename,
            ArchiveFile::Video { filename, .. } => filename,
            ArchiveFile::File { filename, .. } => filename,
        }
    }
    pub fn is_image(&self) -> bool {
        matches!(self, ArchiveFile::Image { .. })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveByType {
    Fanbox,
}

//MarkDown
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveContent {
    Text(String),
    Image(String),
    Video(String),
    File(String),
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveComment {
    pub user: String,
    pub text: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub replies: Vec<ArchiveComment>,
}
