use std::mem;

use chrono::DateTime;
use indicatif::ProgressBar;
use log::{debug, error, info};
use plyne::{Input, Output};
use post_archiver_utils::{Error, Result, Strategy, TaskSet};
use tokio::sync::Mutex;

use crate::{
    api::PatreonClient,
    config::{Config, IncOnDrop, ProgressSet},
    context::Context,
    patreon::{Member, User},
    post::PostEvent,
    Manager,
};

pub async fn list_members(
    campaign_pipeline: Input<Member>,
    user: &User,
    config: &Config,
    client: &PatreonClient,
    progress: &ProgressSet,
) {
    info!("Loading Member List");
    let Ok(mut members) = client.get_members(user).await else {
        error!("Failed to load user data");
        return;
    };

    let total = members.len();
    members.retain(|c| config.filter_member(c));
    let filtered = members.len();
    let excluded = total - filtered;
    info!("");
    info!("Total: {total} members");
    info!("Excluded: {excluded} members");
    info!("Included: {filtered} members");
    info!("");

    if log::log_enabled!(log::Level::Info) {
        display_members(&members);
    }

    progress.creators.inc_length(filtered as u64);
    for member in members {
        campaign_pipeline.send(member).unwrap();
    }
}

pub fn display_members(members: &[Member]) {
    let mut members = members.to_vec();
    members.sort_by(|a, b| a.campaign.name.cmp(&b.campaign.name));

    let (mut id_width, mut cents_width, mut currency_width) = (11_usize, 6_usize, 0_usize);
    for member in members.iter() {
        id_width = member.campaign.id.len().max(id_width);
        cents_width = member.cents().to_string().len().max(cents_width);
        currency_width = member.campaign_currency.len().max(currency_width);
    }

    let cents_total_width = cents_width + 1 + currency_width;
    info!(
        "+-{:-<id_width$}-+-{:-<cents_total_width$}-+-{}------- - -",
        " CreatorId ", " Amount ", " Name "
    );
    for member in members.iter() {
        info!(
            "| {:id_width$} | {:cents_width$.2} {} | {}",
            member.campaign.id,
            member.cents() as f32 / 100.0,
            member.campaign_currency,
            member.campaign.name
        );
    }
    info!(
        "+-{}-+-{}-+-------------- - -",
        "-".to_string().repeat(id_width),
        "-".to_string().repeat(cents_total_width)
    );
    info!("");
}

pub async fn creator_posts(
    mut campaign_pipeline: Output<Member>,
    posts_pipeline: Input<PostEvent>,
    user: &User,
    config: &Config,
    client: &PatreonClient,
    manager: &Manager,
    context: &Mutex<Context>,
    progress: &ProgressSet,
) {
    let tasks: TaskSet<(String, i64, u32), Error> = TaskSet::new("creators").limit(2);

    while let Some(member) = campaign_pipeline.recv().await {
        let guard = IncOnDrop(progress.creators.clone());
        let campaign_id = member.campaign.id.clone();

        let (last_published, failed) = {
            let mut ctx = context.lock().await;
            let record = ctx.campaigns.entry(campaign_id.clone()).or_default();
            (record.last_published(member.cents()), mem::take(&mut record.failed))
        };

        // Retry posts that failed on earlier runs. Failures are merged back by `posts`.
        progress.posts.inc_length(failed.len() as u64);
        for id in failed {
            let campaign = campaign_id.clone();
            posts_pipeline.send(PostEvent::Retry { campaign, id }).unwrap();
        }

        tasks
            .spawn(list_campaign_posts(
                posts_pipeline.clone(),
                client.clone(),
                user.clone(),
                config.clone(),
                manager.clone(),
                member,
                last_published,
                progress.posts.clone(),
                guard,
            ))
            .await;
    }

    let results = tasks.join_all().await;
    let mut ctx = context.lock().await;
    for (campaign, published, cents) in results.into_iter().flatten() {
        ctx.campaigns.entry(campaign).or_default().update(published, cents);
    }
    progress.creators.finish();
}

async fn list_campaign_posts(
    posts_pipeline: Input<PostEvent>,
    client: PatreonClient,
    user: User,
    config: Config,
    manager: Manager,
    member: Member,
    last_published: Option<i64>,
    post_pb: ProgressBar,
    _guard: IncOnDrop,
) -> Result<(String, i64, u32)> {
    let campaign_id = member.campaign.id.clone();
    let increment = matches!(config.strategy(), Strategy::Increment);
    // Patreon can't filter by date, so increment mode pages from newest to oldest
    // and stops at the last seen post.
    let last_published = last_published.filter(|_| increment);

    info!("Loading posts of campaign {campaign_id}");
    let mut next_url = Some(client.get_posts_url(&user, &campaign_id));
    let mut max_timestamp = 0i64;
    let mut total = 0u64;

    while let Some(url) = next_url.take() {
        let (posts, next) = client
            .get_posts(&url)
            .await
            .inspect_err(|e| error!("Failed to load posts of campaign {campaign_id}: {e}"))?;

        let mut stop = false;
        let mut count = 0;
        for post in posts {
            let published = DateTime::parse_from_rfc3339(&post.published_at)
                .map(|t| t.timestamp())
                .unwrap_or_default();
            max_timestamp = max_timestamp.max(published);

            if last_published.is_some_and(|t| published <= t) {
                stop = true;
                continue;
            }

            if !config.filter_post(&post) {
                continue;
            }

            if increment {
                let mut manager = manager.lock().await;
                if let Ok(Some(_)) = manager.importer().lookup_post(&post.url) {
                    continue;
                }
            }

            posts_pipeline.send(PostEvent::Listed(Box::new(post))).unwrap();
            count += 1;
        }
        post_pb.inc_length(count);
        total += count;

        if stop {
            debug!("Reached last archived post of campaign {campaign_id}");
        } else {
            next_url = next;
        }
    }

    info!("Found {total} posts ({campaign_id})");
    Ok((campaign_id, max_timestamp, member.cents()))
}
