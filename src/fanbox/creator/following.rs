use serde::{Deserialize, Serialize};

use crate::fanbox::common::{PostType, User};

use super::Creator;

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FollowingCreator {
    user: User,
    creator_id: String,
    description: String,
    has_adult_content: bool,
    cover_image_url: Option<String>,
    profile_links: Vec<String>,
    profile_items: Vec<ProfileItem>,
    is_followed: bool,
    is_supported: bool,
    is_stopped: bool,
    is_accepting_request: bool,
    has_booth_shop: bool,
}

impl FollowingCreator {
    pub fn name(&self) -> &str {
        &self.user.name
    }
    pub fn creator_id(&self) -> &str {
        &self.creator_id
    }
}

impl From<FollowingCreator> for Creator {
    fn from(creator: FollowingCreator) -> Self {
        Creator {
            creator_id: creator.creator_id,
            user: creator.user,
            fee: 0,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProfileItem {
    id: String,
    #[serde(rename = "type")]
    ty: PostType,
    image_url: String,
    thumbnail_url: String,
}
