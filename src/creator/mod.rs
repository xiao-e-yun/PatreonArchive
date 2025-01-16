use std::{collections::HashSet, error::Error, ops::Deref};

use chrono::{DateTime, Utc};
use log::info;
use post_archiver::{Author, AuthorId, FileMetaId, Link};
use rusqlite::{params, Connection, OptionalExtension};

use crate::{api::fanbox::FanboxClient, config::Config, fanbox::Creator};

pub async fn get_creators(config: &Config) -> Result<Vec<Creator>, Box<dyn Error>> {
    let accepts = config.accepts();
    info!("Accepts:");
    for accept in accepts.list() {
        info!(" + {}", accept);
    }
    info!("");

    let client = FanboxClient::new(&config);
    let mut creators: HashSet<Creator> = HashSet::new();
    info!("Checking creators");
    if accepts.accept_following() {
        let following = client.get_following_creators().await?;
        info!(" + Following: {} found", following.len());
        creators.extend(following.into_iter().map(|f| f.into()));
    }

    if accepts.accept_supporting() {
        let supporting = client.get_supporting_creators().await?;
        info!(" + Supporting: {} found", supporting.len());
        creators.extend(supporting.into_iter().map(|f| f.into()));
    }
    info!("");

    let total = creators.len();
    info!("Total: {} creators", total);
    creators.retain(|c| config.filter_creator(&c));
    let filtered = creators.len();
    info!("Excluded: {} creators", total - filtered);
    info!("Filtered: {} creators", filtered);
    info!("");
    Ok(creators.into_iter().collect())
}

pub fn display_creators(creators: &Vec<Creator>) {
    if log::log_enabled!(log::Level::Info) {
        let mut creators = creators.clone();
        creators.sort_by(|a, b| a.id().cmp(b.id()));

        let (mut id_width, mut fee_width) = (11_usize, 5_usize);
        for creator in creators.iter() {
            id_width = creator.id().len().max(id_width);
            fee_width = creator.fee().to_string().len().max(fee_width);
        }

        info!(
            "+-{:-<id_width$}-+-{:-<fee_width$}--+-{}------- - -",
            " CreatorId ", " Fee ", " Name "
        );
        for creator in creators.iter() {
            info!(
                "| {:id_width$} | {:fee_width$}$ | {}",
                creator.id(),
                creator.fee(),
                creator.name()
            );
        }
        info!(
            "+-{}-+-{}--+------------ - -",
            "-".to_string().repeat(id_width),
            "-".to_string().repeat(fee_width)
        );
        info!("");
    }
}

pub fn sync_creators(
    conn: &mut Connection,
    creators: Vec<Creator>,
) -> Result<Vec<SyncedCreator>, Box<dyn Error>> {
    let mut list = vec![];
    let tx = conn.transaction().unwrap();
    {
        let mut get_alias_stmt = tx.prepare("SELECT target FROM author_alias WHERE source = ?")?;
        let mut get_author_stmt = tx.prepare("SELECT * FROM authors WHERE id = ?")?;
        let mut update_author_stmt = tx.prepare("UPDATE authors SET links = ? WHERE id = ?")?;
        let mut insert_author_stmt =
            tx.prepare("INSERT INTO authors (name,links) VALUES (?,?) RETURNING *")?;
        let mut insert_alias_stmt =
            tx.prepare("INSERT INTO author_alias (source,target) VALUES (?,?)")?;

        for creator in creators {
            let alias = format!("fanbox:{}", creator.id());
            let link = || Link::new("fanbox", &format!("https://{}.fanbox.cc/", creator.id()));

            let author = match get_alias_stmt
                .query_row([&alias], |row| row.get::<_, u32>(0))
                .optional()?
            {
                Some(id) => {
                    // it should be safe to unwrap here
                    // because author_alias has foreign key constraint
                    let mut author = get_author_stmt.query_row([id], row_to_author).unwrap();

                    let links = &mut author.links;
                    let link = link();

                    if !links.contains(&link) {
                        info!(" + Update author `{}` links", author.name);
                        links.push(link);
                        links.sort();
                        let links = serde_json::to_string(&links)?;
                        update_author_stmt.execute(params![links, author.id.raw()])?;
                    }

                    author
                }
                None => {
                    info!(
                        " + Add new creator {} -> `{}`",
                        creator.id(),
                        creator.name()
                    );
                    let name = creator.name();
                    let link = link();
                    let links = serde_json::to_string(&[link])?;
                    let author =
                        insert_author_stmt.query_row(params![name, links], row_to_author)?;
                    insert_alias_stmt
                        .execute(params![alias, author.id.raw()])
                        .unwrap();
                    author
                }
            };

            fn row_to_author(row: &rusqlite::Row) -> Result<Author, rusqlite::Error> {
                let id: AuthorId = AuthorId::new(row.get("id")?);

                let name: String = row.get("name")?;

                let links: String = row.get("links")?;
                let links: Vec<Link> =
                    serde_json::from_str(&links).expect("Author links is not valid JSON");

                let thumb: Option<u32> = row.get("id")?;
                let thumb: Option<FileMetaId> = thumb.map(FileMetaId::new);

                let updated: DateTime<Utc> = row.get("updated")?;

                Ok(Author {
                    id,
                    name,
                    links,
                    thumb,
                    updated,
                })
            }

            list.push(SyncedCreator { creator, author });
        }
    }
    tx.commit().unwrap();
    Ok(list)
}

pub struct SyncedCreator {
    creator: Creator,
    author: Author,
}

impl SyncedCreator {
    pub fn creator(&self) -> &Creator {
        &self.creator
    }
    pub fn author(&self) -> &Author {
        &self.author
    }
}

impl Deref for SyncedCreator {
    type Target = Creator;

    fn deref(&self) -> &Self::Target {
        &self.creator
    }
}
