mod api;
mod config;
mod creator;
mod post;

pub mod fanbox;

use std::error::Error;

use config::Config;
use creator::{display_creators, get_creators, sync_creators};
use log::info;
use post::{filter_unsynced_posts, get_or_insert_tag, get_post_urls, get_posts, sync_posts};
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
    let tag = get_or_insert_tag(&mut conn)?;
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
            sync_posts(&mut conn, &config, tag, &creator, posts).await?;
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

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {

//     let mut authors = unit!("Get Author List", get_author_list(&config).await?);

//     if log::log_enabled!(log::Level::Info) {
//         info!("Save Authors:");
//         authors.sort();
//         let (mut id_width, mut fee_width) = (9_usize, 3_usize);
//         for author in authors.iter() {
//             id_width = author.id().len().max(id_width);
//             fee_width = author.fee().to_string().len().max(fee_width);
//         }

//         info!(
//             "+-{:-<id_width$}-|-{:-<fee_width$}--|-{}-- - -",
//             "CreatorId", "Fee", "Name"
//         );
//         for author in authors.iter() {
//             info!(
//                 "| {:id_width$} | {:fee_width$}$ | {}",
//                 author.id(),
//                 author.fee(),
//                 author.name()
//             );
//         }
//         info!(
//             "+-{}-|-{}--|------- - -",
//             "-".to_string().repeat(id_width),
//             "-".to_string().repeat(fee_width)
//         );
//         info!("");
//     }

//     let posts = unit!(
//         "Get Post List",
//         get_post_list(authors.clone(), &config).await?
//     );
//     info!("New posts: {}", posts.len());
//     info!("");

//     let posts = unit!("Get Posts", get_posts(posts, &config).await?);

//     let (authors, posts, files) = unit!("Resolve", resolve(authors, posts, &config))?;

//     unit!("Build", build(authors, posts, files, &config).await?);

//     Ok(())
// }
