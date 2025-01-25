mod api;
mod config;
mod creator;
mod post;

pub mod fanbox;

use std::error::Error;

use config::Config;
use creator::{display_creators, get_creators, sync_creators};
use log::info;
use post::{filter_unsynced_posts, get_or_insert_free_tag, get_post_urls, get_posts, sync_posts};
use rusqlite::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();
    config.init_logger();
    info!("# Fanbox Archive #");
    info!("");

    let mut conn = create_connection(&config)?;

    info!("Loading Creator List");
    let creators = get_creators(&config).await?;
    display_creators(&creators);

    info!("Syncing Creator List");
    let creators = sync_creators(&mut conn, creators)?;

    info!("Loading Creators Post");
    let fanbox_tag = get_or_insert_free_tag(&mut conn,"fanbox")?;
    let free_tag = get_or_insert_free_tag(&mut conn,"free")?;
    for creator in creators {
        info!("{}", creator.id());
        let posts = get_post_urls(&config, creator.creator()).await?;
        let posts = if config.force() {
            info!("{} posts", posts.len());
            posts
        } else {
            let total_post = posts.len();
            let posts: Vec<fanbox::PostListItem> = filter_unsynced_posts(&mut conn, posts)?;
            info!("{} posts, {} unsynced", total_post, posts.len());
            posts
        };

        let posts = get_posts(&config, posts).await?;
        if !posts.is_empty() {
            sync_posts(&mut conn, &config, &creator, posts, (fanbox_tag,free_tag)).await?;
        }

        info!("");
    }

    info!("All done!");
    Ok(())
}

pub fn create_connection(config: &Config) -> Result<rusqlite::Connection, rusqlite::Error> {
    let db_path = config.output().join("post-archiver.db");
    let conn = if db_path.exists() {
        info!("Connecting to database: {}", db_path.display());
        Connection::open(&db_path)?
    } else {
        info!("Creating database: {}", db_path.display());
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create database directory");
        }

        let conn = Connection::open(&db_path)?;
        conn.execute_batch(post_archiver::utils::TEMPLATE_DATABASE_UP_SQL)?;
        conn
    };

    Ok(conn)
}