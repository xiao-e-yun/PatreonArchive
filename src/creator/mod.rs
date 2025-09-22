use std::sync::Arc;

use log::{error, info};
use post_archiver::{
    importer::{UnsyncAlias, UnsyncAuthor},
    manager::PostArchiverManager,
    AuthorId, PlatformId,
};
use post_archiver_utils::Result;

use crate::{
    patreon::{Campaign, Member},
    CampaignPipelineInput, Client, Config, User,
};

pub async fn list_members(
    user: User,
    config: Config,
    client: Client,
    campaign_pipeline: CampaignPipelineInput,
) {
    info!("Loading Member List");
    let Ok(mut members) = client.get_members(&user).await else {
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

    for member in members {
        let campaign = member.campaign.id.clone();
        campaign_pipeline.send(campaign).unwrap();
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

pub fn sync_campaign(
    manager: &PostArchiverManager,
    platform: PlatformId,
    campaign: &Arc<Campaign>,
) -> Result<AuthorId> {
    let alias = UnsyncAlias::new(platform, campaign.id.clone()).link(campaign.url.clone());

    let author = UnsyncAuthor::new(campaign.name.clone())
        .aliases(vec![alias])
        .sync(manager)?;
    info!("Campaign imported: {} ({})", campaign.name, campaign.id);

    Ok(author)
}
