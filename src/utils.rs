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

pub fn cyrb53(str: &str) -> String {
    let seed: u64 = 1;
    let mut h1 = 0xdeadbeef ^ seed;
    let mut h2 = 0x41c6ce57 ^ seed;
    for ch in str.chars() {
        let code = ch as u64;
        h1 = (h1 ^ code).wrapping_mul(2654435761);
        h2 = (h2 ^ code).wrapping_mul(1597334677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2246822507);
    h1 ^= (h2 ^ (h2 >> 13)).wrapping_mul(3266489909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2246822507);
    h2 ^= (h1 ^ (h1 >> 13)).wrapping_mul(3266489909);

    format!("{:x}", (4294967296 * (2097151 & h2) + h1))
}
