mod api;
mod config;
mod creator;
mod post;

pub mod fanbox;

use std::error::Error;

use config::Config;
use creator::{display_creators, get_creators, sync_creators};
use log::info;
use post::{filter_unsynced_posts, get_post_urls, get_posts, sync_posts};
use post_archiver::importer::PostArchiverImporter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();
    config.init_logger();
    info!("# Fanbox Archive #");
    info!("");

    if !config.output().exists() {
        info!("Creating output folder");
        std::fs::create_dir_all(config.output())?;
    }

    let mut importer = PostArchiverImporter::open_or_create(config.output())?;

    info!("Loading Creator List");
    let creators = get_creators(&config).await?;
    display_creators(&creators);

    info!("Syncing Creator List");
    let authors = sync_creators(&mut importer, creators)?;

    info!("Loading Creators Post");
    for (author, creator_id) in authors {
        info!("{}", &author.name);
        let mut posts = get_post_urls(&config, &creator_id).await?;

        let total_post = posts.len();
        let mut posts_count_info = format!("{} posts", total_post);
        if !config.force() {
            posts = filter_unsynced_posts(&mut importer, posts)?;
            posts_count_info += &format!(" ({} unsynced)", posts.len());
        };
        info!(" + {}", posts_count_info);

        let posts = get_posts(&config, posts).await?;
        if !posts.is_empty() {
            sync_posts(&mut importer, &config, author.id, posts).await?;
        }

        info!("");
    }

    info!("All done!");
    Ok(())
}
