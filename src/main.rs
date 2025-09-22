mod api;
mod config;
mod creator;
mod post;

mod patreon;

use std::{collections::HashMap, error::Error, rc::Rc};

use api::PatreonClient;
use creator::list_members;
use log::{info, warn};
use patreon::{comment::Comment, post::Post};
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
    let manager = PostArchiverManager::open_or_create(config.output())?;

    PatreonSystem::new(Rc::new(Mutex::new(manager)), config, client, user)
        .execute()
        .await;
    Ok(())
}

define_tasks! {
    PatreonSystem
    pipelines {
        CampaignPipeline: String,
        PostsPipeline: (Post, Vec<Comment>, oneshot::Receiver<HashMap<String, TempPath>>),
        FilesPipeline: (Vec<String>, oneshot::Sender<HashMap<String, TempPath>>),
    }
    vars {
        Manager: Rc<Mutex<PostArchiverManager>>,
        Config: config::Config,
        Client: PatreonClient,
        User: patreon::User,
    }
    tasks {
        list_members,
        list_posts,
        download_files,
        sync_posts,
    }
}
