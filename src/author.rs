use std::{collections::HashMap, error::Error};

use log::info;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio::task::JoinSet;
use url::Url;

use crate::{
    api::APIResponse,
    config::{Config, SaveType},
    utils::{PostType, RequestInner, User},
};

pub async fn get_author_list(config: &Config) -> Result<Vec<Author>, Box<dyn Error>> {
    let save_types = config.save_types();
    fn check(save_types: &SaveType, ty: SaveType) -> bool {
        save_types == &SaveType::All || save_types == &ty
    }

    let mut saved = HashMap::new();
    let mut awaits = JoinSet::new();

    if check(&save_types, SaveType::Following) {
        let following = get_following_authors(config.clone());
        info!("Checking following authors");
        awaits.spawn(following);
    }

    if check(&save_types, SaveType::Supporting) {
        let supporting = get_supporting_authors(config.clone());
        info!("Checking supporting authors");
        awaits.spawn(supporting);
    }

    while let Some(res) = awaits.join_next().await {
        let (has_fee, result) = res??;
        for author in result {
            if !config.filter_creator(&author.creator_id) {
                continue;
            }
            
            if !has_fee && saved.contains_key(&author.creator_id) {
                continue;
            }
            saved.insert(author.creator_id.clone(), author);
        }
    }

    Ok(saved.into_iter().map(|(_, v)| v).collect())
}

pub async fn get_following_authors(config: Config) -> Result<(bool, Vec<Author>), String> {
    let url = Url::parse("https://api.fanbox.cc/creator.listFollowing").unwrap();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Origin", "https://www.fanbox.cc")
        .header("Cookie", config.session())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    match response {
        APIResponse::ListFollowing(RequestInner { body }) => {
            Ok((false, body.into_iter().map(|f| f.into()).collect()))
        }
        APIResponse::Error { error } => Err(format!("{} (tips: check your session)", error).into()),
        _ => unreachable!(),
    }
}

pub async fn get_supporting_authors(config: Config) -> Result<(bool, Vec<Author>), String> {
    let url = Url::parse("https://api.fanbox.cc/plan.listSupporting").unwrap();

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("Origin", "https://www.fanbox.cc")
        .header("Cookie", config.session())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    match response {
        APIResponse::ListSupporting(RequestInner { body }) => {
            Ok((true, body.into_iter().map(|f| f.into()).collect()))
        }
        APIResponse::Error { error } => Err(format!("{} (tips: check your session)", error)),
        _ => unreachable!(),
    }
}

//===================================================
// Type
//===================================================
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct Author {
    creator_id: String,
    user: User,
    fee: u32,
}

impl Author {
    pub fn id(&self) -> &str {
        &self.creator_id
    }
    pub fn name(&self) -> String {
        self.user.name.clone()
    }
    pub fn fee(&self) -> u32 {
        self.fee
    }
}

impl PartialEq for Author {
    fn eq(&self, other: &Self) -> bool {
        self.creator_id == other.creator_id
    }
}
impl Eq for Author {}
impl PartialOrd for Author {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.creator_id.partial_cmp(&other.creator_id)
    }
}
impl Ord for Author {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.creator_id.cmp(&other.creator_id)
    }
}

impl From<FollowingAuthor> for Author {
    fn from(author: FollowingAuthor) -> Self {
        Author {
            creator_id: author.creator_id,
            user: author.user,
            fee: 0,
        }
    }
}

impl From<SupportingAuthor> for Author {
    fn from(author: SupportingAuthor) -> Self {
        Author {
            creator_id: author.creator_id,
            user: author.user,
            fee: author.fee,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FollowingAuthor {
    user: User,
    creator_id: String,
    description: String,
    has_adult_content: bool,
    cover_image_url: Option<Url>,
    profile_links: Vec<Url>,
    profile_items: Vec<ProfileItem>,
    is_followed: bool,
    is_supported: bool,
    is_stopped: bool,
    is_accepting_request: bool,
    has_booth_shop: bool,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SupportingAuthor {
    #[serde_as(as = "DisplayFromStr")]
    id: String,
    title: String,
    fee: u32,
    description: String,
    cover_image_url: Option<Url>,
    user: User,
    creator_id: String,
    has_adult_content: bool,
    payment_method: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProfileItem {
    id: String,
    #[serde(rename = "type")]
    ty: PostType,
    image_url: Url,
    thumbnail_url: Url,
}
