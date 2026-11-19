pub mod save_type;

use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use dotenv::dotenv;
use indicatif::{MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use post_archiver_utils::{define_progress, Strategy};
use std::{num::NonZeroU32, path::PathBuf};

use crate::patreon::{post::Post, Member};

#[derive(Debug, Clone, Parser)]
pub struct Config {
    /// Your `session_id` cookie
    #[clap(env = "SESSION", hide_env_values = true)]
    session: String,
    /// Which you path want to save
    #[arg(default_value = "./archive", env = "OUTPUT")]
    output: PathBuf,
    /// Archiving strategy (`full` re-archives existing posts)
    #[arg(long, default_value_t)]
    strategy: Strategy,
    /// Whitelist of creator IDs
    #[arg(short, long, num_args = 0..)]
    whitelist: Vec<String>,
    /// Blacklist of creator IDs
    #[arg(short, long, num_args = 0..)]
    blacklist: Vec<String>,
    /// Limit API requests per minute
    #[arg(long, default_value = "60")]
    limit: NonZeroU32,
    /// Skip free post
    #[arg(long, name = "skip-free")]
    skip_free: bool,
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
    #[clap(skip)]
    pub multi: MultiProgress,
}

impl Config {
    /// Parse the configuration from the environment and command line arguments
    pub fn parse() -> Self {
        dotenv().ok();
        <Self as Parser>::parse()
    }
    /// Create a logger with the configured verbosity level
    pub fn init_logger(&self) {
        let mut logger = env_logger::Builder::new();
        logger
            .filter_level(self.verbose.log_level_filter())
            .format_target(false);

        LogWrapper::new(self.multi.clone(), logger.build())
            .try_init()
            .unwrap();
    }
    /// Get the session cookie
    pub fn session(&self) -> String {
        if self.session.starts_with("session_id=") {
            self.session.clone()
        } else {
            format!("session_id={}", self.session)
        }
    }
    pub const fn output(&self) -> &PathBuf {
        &self.output
    }
    pub const fn limit(&self) -> NonZeroU32 {
        self.limit
    }

    pub fn filter_member(&self, member: &Member) -> bool {
        let id = member
            .campaign
            .url
            .split('/')
            .next_back()
            .unwrap()
            .to_string();
        let mut accept = true;

        accept &= !(self.skip_free && member.cents() == 0);
        accept &= self.whitelist.is_empty() || self.whitelist.contains(&id);
        accept &= !self.blacklist.contains(&id);

        accept
    }

    pub fn filter_post(&self, post: &Post) -> bool {
        let mut accept = true;

        // skip_free is true and the post is free
        accept &= !(self.skip_free && post.is_free());
        accept &= post.current_user_can_view;

        accept
    }

    pub const fn strategy(&self) -> Strategy {
        self.strategy
    }
}

define_progress!(ProgressSet [
    creators,
    posts,
    files
]);

/// Increments the progress bar by one when dropped, so early returns are still counted.
pub struct IncOnDrop(pub ProgressBar);

impl Drop for IncOnDrop {
    fn drop(&mut self) {
        self.0.inc(1);
    }
}
