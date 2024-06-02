use std::hash::Hash;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use url::Url;

#[macro_export]
macro_rules! unit {
  ($name:expr, $($tail:tt)*) => {
    {
      log::info!("={}==================================================",$name);
      let now = std::time::Instant::now();
      let value = $($tail)*;
      log::info!("Done  `{}` ({} ms)", $name, now.elapsed().as_millis());
      log::info!("");
      value
    }
  };
}

#[macro_export]
macro_rules! unit_short {
  ($name:expr, $($tail:tt)*) => {
    {
      log::info!("* {}",$name);
      let now = std::time::Instant::now();
      let value = $($tail)*;
      log::info!("Done  `{}` ({} ms)", $name, now.elapsed().as_millis());
      value
    }
  };
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct RequestInner<T> {
    pub body: T,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct User {
    icon_url: Option<Url>,
    pub name: String,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: u32,
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