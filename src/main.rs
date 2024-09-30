#![feature(closure_track_caller)]

pub mod api;
pub mod author;
pub mod config;
pub mod post;

pub mod resolve;
pub mod utils;


#[cfg(test)]
mod test;

use std::error::Error;

use author::get_author_list;
use config::Config;
use log::info;
use post::{get_post_list, get_posts};
use resolve::{build, resolve};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();

    let mut authors = unit!("Get Author List", get_author_list(&config).await?);

    if log::log_enabled!(log::Level::Info) {
        info!("Save Authors:");
        authors.sort();
        let (mut id_width, mut fee_width) = (9_usize, 3_usize);
        for author in authors.iter() {
            id_width = author.id().len().max(id_width);
            fee_width = author.fee().to_string().len().max(fee_width);
        }

        info!(
            "+-{:-<id_width$}-|-{:-<fee_width$}--|-{}-- - -",
            "CreatorId", "Fee", "Name"
        );
        for author in authors.iter() {
            info!(
                "| {:id_width$} | {:fee_width$}$ | {}",
                author.id(),
                author.fee(),
                author.name()
            );
        }
        info!(
            "+-{}-|-{}--|------- - -",
            "-".to_string().repeat(id_width),
            "-".to_string().repeat(fee_width)
        );
        info!("");
    }

    let posts = unit!(
        "Get Post List",
        get_post_list(authors.clone(), &config).await?
    );
    info!("New posts: {}", posts.len());
    info!("");

    let posts = unit!("Get Posts", get_posts(posts, &config).await?);

    let (authors, posts, files) = unit!("Resolve", resolve(authors, posts, &config))?;

    unit!("Build", build(authors, posts, files, &config).await?);

    Ok(())
}
