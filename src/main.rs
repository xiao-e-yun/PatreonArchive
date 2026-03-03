#![allow(clippy::too_many_arguments)]

mod api;
mod config;
mod context;
mod creator;
mod post;

mod patreon;

use std::{collections::HashMap, error::Error};

use api::PatreonClient;
use config::{Config, ProgressSet};
use context::Context;
use creator::list_members;
use log::{info, warn};
use patreon::{comment::Comment, post::Post, Member, User};
use plyne::define_tasks;
use post::{file::download_files, list_posts, sync_posts};
use post_archiver::{manager::PostArchiverManager, utils::VERSION};
use post_archiver_utils::display_metadata;
use tempfile::TempPath;
use tokio::sync::{oneshot, Mutex};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = config::Config::parse();
    config.init_logger();

    display_metadata(
        "Patreon Archive",
        &[
            ("PostArchiver", VERSION),
            ("Strategy", config.strategy().as_str()),
            ("Output", config.output().to_str().unwrap()),
        ],
    );

    if !config.output().exists() {
        warn!("Creating output folder");
        std::fs::create_dir_all(config.output())?;
    }

    let client = PatreonClient::new(&config);

    info!("Checking User Data");
    let user = client.get_current_user_id().await?;
    info!("= User ===========================");
    info!("Name: {}", user.full_name);
    info!("Id: {}", user.id);
    info!("==================================");
    info!("");

    info!("Connecting to PostArchiver");
    let output = config.output().clone();
    let manager = PostArchiverManager::open_or_create(&output)?;

    let context = context::Context::load(&output);
    let manager = Mutex::new(manager);

    let progress = ProgressSet::new(&config);

    let PatreonSystemContext { context, .. } =
        PatreonSystem::new(manager, config, client, user, context.clone(), progress)
            .execute()
            .await;

    info!("All done!");

    context.save(&output);
    Ok(())
}

pub type PostsEvent = (
    Post,
    Vec<Comment>,
    oneshot::Receiver<HashMap<String, TempPath>>,
);
pub type FilesEvent = (Vec<String>, oneshot::Sender<HashMap<String, TempPath>>);

pub type Manager = Mutex<PostArchiverManager>;

define_tasks! {
    PatreonSystem
    pipelines {
        campaign_pipeline: Member,
        posts_pipeline: PostsEvent,
        files_pipeline: FilesEvent,
    }
    vars {
        manager: Manager,
        config: Config,
        client: PatreonClient,
        user: User,
        context: Context,
        progress_set: ProgressSet,
    }
    tasks {
        list_members,
        list_posts,
        download_files,
        sync_posts,
    }
}
