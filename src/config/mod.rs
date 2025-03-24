pub mod save_type;

use clap::{arg, Parser};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use dotenv::dotenv;
use save_type::SaveType;
use std::path::PathBuf;

use crate::fanbox::{Creator, PostListItem};

#[derive(Debug, Clone, Parser, Default)]
pub struct Config {
    /// Your `FANBOXSESSID` cookie
    #[clap(env = "FANBOXSESSID")]
    session: String,
    /// Which you path want to save
    #[arg(default_value = "./archive", env = "OUTPUT")]
    output: PathBuf,
    /// Which you type want to save
    #[arg(short, long, default_value = "supporting", env = "SAVE")]
    save: SaveType,
    /// Force download
    #[arg(short, long)]
    force: bool,
    /// Overwrite existing files
    #[arg(short, long)]
    overwrite: bool,
    /// Whitelist of creator IDs
    #[arg(short, long, num_args = 0..)]
    whitelist: Vec<String>,
    /// Blacklist of creator IDs
    #[arg(short, long, num_args = 0..)]
    blacklist: Vec<String>,
    /// Limit download concurrency
    #[arg(long, default_value = "5")]
    limit: usize,
    /// Skip free post
    #[arg(long, name = "skip-free")]
    skip_free: bool,
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

impl Config {
    /// Parse the configuration from the environment and command line arguments
    pub fn parse() -> Self {
        dotenv().ok();
        <Self as Parser>::parse()
    }
    /// Create a logger with the configured verbosity level
    pub fn init_logger(&self) -> () {
        env_logger::Builder::new()
            .filter_level(self.verbose.log_level_filter())
            .format_target(false)
            .init();
    }
    /// Get the session cookie
    pub fn session(&self) -> String {
        if self.session.starts_with("FANBOXSESSID=") {
            self.session.clone()
        } else {
            format!("FANBOXSESSID={}", self.session)
        }
    }
    pub fn overwrite(&self) -> bool {
        self.overwrite
    }
    pub fn accepts(&self) -> SaveType {
        self.save
    }

    pub fn output(&self) -> &PathBuf {
        &self.output
    }
    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn filter_creator(&self, creator: &Creator) -> bool {
        let creator_id = creator.creator_id.to_string();
        let mut accept = true;

        accept &= !(self.skip_free && creator.fee == 0);
        accept &= self.whitelist.is_empty() || self.whitelist.contains(&creator_id);
        accept &= !self.blacklist.contains(&creator_id);

        accept
    }

    pub fn filter_post(&self, post: &PostListItem) -> bool {
        let mut accept = true;

        // skip_free is true and the post is free
        accept &= !(self.skip_free && post.fee_required == 0);
        // is_restricted means the post is for supporters only
        accept &= !post.is_restricted;

        accept
    }

    pub fn force(&self) -> bool {
        self.force
    }
}
