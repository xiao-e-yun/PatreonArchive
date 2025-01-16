use serde::{Deserialize, Serialize};

use crate::fanbox::common::User;

use super::Creator;

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SupportingCreator {
    pub id: String,
    pub title: String,
    pub fee: u32,
    pub description: String,
    pub cover_image_url: Option<String>,
    pub user: User,
    pub creator_id: String,
    pub has_adult_content: bool,
    pub payment_method: String,
}

impl From<SupportingCreator> for Creator {
    fn from(creator: SupportingCreator) -> Self {
        Creator {
            creator_id: creator.creator_id,
            user: creator.user,
            fee: creator.fee,
        }
    }
}
