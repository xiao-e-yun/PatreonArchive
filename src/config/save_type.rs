use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Hash, ValueEnum, PartialEq, Eq, Default)]
pub enum SaveType {
    All,
    Following,
    #[default]
    Supporting,
}

#[allow(unused)]
impl SaveType {
    pub fn accept_all(&self) -> bool {
        *self == Self::All
    }
    pub fn accept_following(&self) -> bool {
        *self == Self::Following || self.accept_all()
    }
    pub fn accept_supporting(&self) -> bool {
        *self == Self::Supporting || self.accept_all()
    }
    pub fn list(&self) -> Vec<&'static str> {
        match self {
            Self::All => vec!["following", "supporting"],
            Self::Following => vec!["following"],
            Self::Supporting => vec!["supporting"],
        }
    }
}

impl fmt::Display for SaveType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Following => write!(f, "following"),
            Self::Supporting => write!(f, "supporting"),
        }
    }
}
