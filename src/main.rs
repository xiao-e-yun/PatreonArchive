#![allow(clippy::too_many_arguments)]

mod api;
mod config;
mod context;
mod creator;
mod post;

mod patreon;

use std::{error::Error, sync::Arc};

use api::PatreonClient;
use config::{Config, ProgressSet};
use context::Context;
use creator::{creator_posts, list_members};
use log::info;
use patreon::{Member, User};
use plyne::define_tasks;
use post::{posts, PostEvent};
use post_archiver::{manager::PostArchiverManager, utils::VERSION};
use post_archiver_utils::{display_metadata, setup_database};
use tokio::sync::Mutex;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = config::Config::parse();
    config.init_logger();

    display_metadata!("Patreon Archive" {
        "Version": format!("v{}", env!("CARGO_PKG_VERSION")),
        "PostArchiver Version": VERSION,
        "Strategy": config.strategy(),
        "Limit": format!("{}/min", config.limit()),
        "Output": config.output().display(),
    });

    let client = PatreonClient::new(&config);

    info!("Checking User Data");
    let user = client.get_current_user_id().await?;
    info!("= User ===========================");
    info!("Name: {}", user.full_name);
    info!("Id: {}", user.id);
    info!("==================================");
    info!("");

    let output = config.output().clone();
    let manager = Arc::new(Mutex::new(setup_database(&output)?));
    let context = Mutex::new(Context::load(&output));
    let progress = ProgressSet::new(&config.multi);

    let system = PatreonSystem::new(manager, config, client, user, context, progress);
    let PatreonSystemContext { context, .. } = system.execute().await;

    info!("All done!");

    context.into_inner().save(&output);
    Ok(())
}

pub type Manager = Arc<Mutex<PostArchiverManager>>;

define_tasks! {
    PatreonSystem
    pipelines {
        campaign_pipeline: Member,
        posts_pipeline: PostEvent,
    }
    vars {
        manager: Manager,
        config: Config,
        client: PatreonClient,
        user: User,
        context: Mutex<Context>,
        progress: ProgressSet,
    }
    tasks {
        list_members,
        creator_posts,
        posts,
    }
}
