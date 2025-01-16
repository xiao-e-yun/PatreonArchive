use std::hash::Hash;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub icon_url: Option<String>,
    pub name: String,
    pub user_id: String,
}

impl User {
    pub fn id(&self) -> &str {
        &self.user_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
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
