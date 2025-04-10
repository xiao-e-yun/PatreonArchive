use std::error::Error;

use log::info;
use post_archiver::{importer::UnsyncAuthor, manager::PostArchiverManager, Author, Link};
use rusqlite::Connection;

use crate::{api::patreon::PatreonClient, config::Config, patreon::{Member, User}};

pub async fn get_user_and_members(config: &Config) -> Result<(User, Vec<Member>), Box<dyn Error>> {
    let client = PatreonClient::new(&config);

    info!("Checking User Data");
    let user = client.get_current_user_id().await?;
    info!("Name: {}", user.full_name);
    info!("Id: {}", user.id);
    info!("");

    info!("Loading Member List");
    let mut members = client.get_members(&user).await?;
    info!("");

    let total = members.len();
    info!("Total: {} members", total);
    members.retain(|c| config.filter_member(c));
    let filtered = members.len();
    info!("Excluded: {} members", total - filtered);
    info!("Included: {} members", filtered);
    info!("");
    Ok((user, members.into_iter().collect()))
}

pub fn display_members(members: &[Member]) {
    if log::log_enabled!(log::Level::Info) {
        let mut members = members.to_vec();
        members.sort_by(|a, b| a.campaign.name.cmp(&b.campaign.name));

        let (mut id_width, mut cents_width, mut currency_width) = (11_usize, 5_usize, 0_usize);
        for member in members.iter() {
            id_width = member.campaign.name.len().max(id_width);
            cents_width =  member.cents().to_string().len().max(cents_width);
            currency_width = member.campaign_currency.len().max(currency_width);
        }

        let cents_total_width = cents_width + 1 + currency_width;
        info!(
            "+-{:-<id_width$}-+-{:-<cents_total_width$}-+-{}------- - -",
            " CreatorId ", " Cents ", " Name "
        );
        for member in members.iter() {
            info!(
                "| {:id_width$} | {:cents_width$} {} | {}",
                member.campaign.id,
                member.cents(),
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
}

pub fn sync_campaign(
    manager: &mut PostArchiverManager<Connection>,
    members: Vec<Member>,
) -> Result<Vec<(Author, String)>, Box<dyn Error>> {
    let mut list = vec![];
    let manager = manager.transaction()?;

    for member in members.into_iter() {
        let alias = format!("patreon:{}", member.campaign.id);
        let link = Link::new("patreon", &member.campaign.url);
        let author = UnsyncAuthor::new(member.campaign.name.clone())
            .alias(vec![alias])
            .links(vec![link])
            .sync(&manager)?;

        list.push((author, member.campaign.id.clone()));
    }

    manager.commit()?;
    Ok(list)
}
