pub mod following;
pub mod supporting;

use std::{hash::Hash, ops::Deref};

use serde::{Deserialize, Serialize};

use super::common::User;

pub use following::*;
pub use supporting::*;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Creator {
    pub creator_id: String,
    pub user: User,
    pub fee: u32,
}

impl Deref for Creator {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

impl Hash for Creator {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.creator_id.hash(state);
    }
}

impl PartialEq for Creator {
    fn eq(&self, other: &Self) -> bool {
        self.creator_id == other.creator_id
    }
}

impl Eq for Creator {}
