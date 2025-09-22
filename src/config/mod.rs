pub mod save_type;

use clap::{arg, Parser};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use dotenv::dotenv;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use std::{ops::Deref, path::PathBuf};

use crate::patreon::{post::Post, Member};

#[derive(Debug, Clone, Parser, Default)]
pub struct Config {
    /// Your `session_id` cookie
    #[clap(env = "SESSION")]
    session: String,
    /// Which you path want to save
    #[arg(default_value = "./archive", env = "OUTPUT")]
    output: PathBuf,
    /// Force download
    #[arg(short, long)]
    force: bool,
    /// Whitelist of creator IDs
    #[arg(short, long, num_args = 0..)]
    whitelist: Vec<String>,
    /// Blacklist of creator IDs
    #[arg(short, long, num_args = 0..)]
    blacklist: Vec<String>,
    /// Limit download concurrency
    #[arg(long, default_value = "20")]
    limit: usize,
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
        logger.filter_level(self.verbose.log_level_filter())
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
    pub const fn limit(&self) -> usize {
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

    pub const fn force(&self) -> bool {
        self.force
    }

    pub fn progress(&self, prefix: &'static str) -> Progress {
        Progress::new(&self.multi, prefix)
    }
}

#[derive(Debug, Clone)]
pub struct Progress(ProgressBar);

impl Progress {
    pub fn new(multi: &MultiProgress, prefix: &'static str) -> Self {
        Self(
            multi.add(
                ProgressBar::new(0)
                    .with_style(Self::style())
                    .with_prefix(format!("[{prefix}]")),
            ),
        )
    }

    fn style() -> ProgressStyle {
        ProgressStyle::with_template("{prefix:.bold.dim} {wide_bar:.cyan/blue} {pos:>3}/{len:3}")
            .unwrap()
            .progress_chars("#>-")
    }
}

impl Deref for Progress {
    type Target = ProgressBar;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSet {
    pub creators: Progress,
    pub posts: Progress,
    pub files: Progress,
}

impl ProgressSet {
    pub fn new(config: &Config) -> Self {
        Self {
            creators: config.progress("creators"),
            posts: config.progress("posts"),
            files: config.progress("files"),
        }
    }
}
