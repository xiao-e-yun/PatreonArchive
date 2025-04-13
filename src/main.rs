mod api;
mod config;
mod creator;
mod post;

mod patreon;

use std::error::Error;

use config::Config;
use creator::{display_members, get_user_and_members, sync_campaign};
use log::{info, warn};
use post::{filter_unsynced_posts, get_posts, sync_posts};
use post_archiver::{manager::PostArchiverManager, utils::VERSION};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();
    config.init_logger();

    info!("# Patreon Archive #");
    info!("");
    info!("==================================");
    info!("PostArchiver version: v{}", VERSION);
    info!("Overwrite: {}", config.overwrite());
    info!("Output: {}", config.output().display());
    info!("==================================");

    if !config.output().exists() {
        warn!("Creating output folder");
        std::fs::create_dir_all(config.output())?;
    }

    info!("Connecting to PostArchiver");
    let mut manager = PostArchiverManager::open_or_create(config.output())?;

    let (user, members) = get_user_and_members(&config).await?;
    display_members(&members);

    info!("Syncing Campaign List");
    let authors = sync_campaign(&mut manager, members)?;

    info!("Loading Members Post");
    for (author, campaign_id) in authors {
        info!("{}", &author.name);
        let mut posts = get_posts(&config, &user, &campaign_id).await?;

        let total_post = posts.len();
        let mut posts_count_info = format!("{} posts", total_post);
        if !config.force() {
            posts = filter_unsynced_posts(&mut manager, posts)?;
            posts_count_info += &format!(" ({} unsynced)", posts.len());
        };
        info!(" + {}", posts_count_info);

        if !posts.is_empty() {
            sync_posts(&mut manager, &config, author.id, posts).await?;
        }

        info!("");
    }

    info!("All done!");
    Ok(())
}
