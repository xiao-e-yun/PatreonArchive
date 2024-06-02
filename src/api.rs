use serde::{Deserialize, Serialize};

use crate::{
    author::{FollowingAuthor, SupportingAuthor},
    post::{Post, PostList},
    utils::RequestInner,
};

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(untagged)]
pub enum APIResponse {
    ListCreator(RequestInner<PostList>),
    ListSupporting(RequestInner<Vec<SupportingAuthor>>),
    ListFollowing(RequestInner<Vec<FollowingAuthor>>),
    Post(RequestInner<Post>),
    Error { error: String },
}
