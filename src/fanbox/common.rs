use std::hash::Hash;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub icon_url: Option<String>,
    pub name: String,
    pub user_id: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PostType {
    Image,
    Text,
    File,
    Article,
    Video,
    Entry,
}
