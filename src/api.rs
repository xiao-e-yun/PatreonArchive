use jsonapi_deserialize::{deserialize_document, Document, JsonApiDeserialize};
use log::{debug, trace};
use post_archiver_utils::{ArchiveClient, Error, Result};
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client,
};
use tempfile::TempPath;

use crate::{
    config::Config,
    patreon::{comment::Comment, post::Post, Member, User},
};

#[derive(Debug, Clone)]
pub struct PatreonClient {
    inner: ArchiveClient,
}

impl PatreonClient {
    pub fn new(config: &Config) -> Self {
        const USER_AGENT: &str =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";

        let limit = config.limit() as u32;
        let inner = ArchiveClient::builder(
            Client::builder()
                .user_agent(USER_AGENT)
                .default_headers(HeaderMap::from_iter(
                    [
                        (header::COOKIE, config.session()),
                        (header::ORIGIN, "https://www.patreon.com".to_owned()),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k, HeaderValue::from_str(&v).unwrap())),
                ))
                .build()
                .unwrap(),
            limit * 60,
        )
        .pre_sec_limit(limit)
        .build();

        Self { inner }
    }

    pub async fn fetch<T: JsonApiDeserialize>(&self, url: &str) -> Result<Document<T>> {
        let request = self.inner.get(url);
        let response = request.send().await?;
        let response = response.text().await?;

        trace!("Fetched {url}");
        deserialize_document(&response).map_err(|e| Error::InvalidResponse(e.to_string()))
    }

    pub async fn download(&self, url: &str) -> Result<TempPath> {
        let path = self.inner.download(url).await?;
        trace!("Downloaded {url}");
        Ok(path)
    }

    pub async fn get_current_user_id(&self) -> Result<User> {
        let url = "https://www.patreon.com/api/current_user?include=[]&fields[user]=id,full_name";
        let list: Document<User> = self.fetch(url).await?;

        Ok(list.data)
    }

    pub async fn get_members(&self, user: &User) -> Result<Vec<Member>> {
        let url = format!("https://www.patreon.com/api/members?include=campaign&fields[campaign]=name,url&filter[user_id]={}&filter[membership_type]=active_patron,declined_patron,free_trial,gifted_c2f,gifted_f2f,free_member&fields[member]=is_free_member,campaign_pledge_amount_cents,campaign_currency&page[offset]=0&page[count]=1000&json-api-version=1.0&json-api-use-default-includes=false", user.id);

        let mut next_url = Some(url);
        let mut list: Vec<Member> = vec![];
        while let Some(url) = next_url {
            let document: Document<Vec<Member>> = self.fetch(&url).await?;

            list.extend(document.data);
            next_url = document.links.unwrap().next.map(|v| v.href.to_string())
        }

        Ok(list)
    }

    pub fn get_posts_url(&self, user: &User, campaign: &str) -> String {
        format!(
            "https://www.patreon.com/api/posts?include=campaign,media,audio.null,audio_preview.null,poll.null,poll.choices&fields[post]=comment_count,content,current_user_can_view,min_cents_pledged_to_view,embed,image,post_metadata,published_at,post_type,title,url&fields[campaign]=name,url&fields[media]=id,image_urls,download_url,metadata,file_name&sort=-published_at&filter[is_draft]=false&filter[accessible_by_user_id]={}&filter[contains_exclusive_posts]=true&json-api-use-default-includes=false&json-api-version=1.0&filter[campaign_id]={}",
            user.id,
            campaign
        )
    }

    pub async fn get_posts(&self, url: &str) -> Result<(Vec<Post>, Option<String>)> {
        let document: Document<Vec<Post>> = self.fetch(url).await?;

        let next_url = document
            .links
            .and_then(|links| links.next.map(|v| v.href.to_string()));

        Ok((document.data, next_url))
    }

    pub async fn get_comments(&self, post_id: &str) -> Result<Vec<Comment>> {
        let url = format!(
            "https://www.patreon.com/api/posts/{post_id}/comments?include=commenter.campaign,replies,replies.commenter,replies.parent&fields[comment]=body,created&fields[user]=image_url,full_name,url&page[count]=1000&sort=-created&json-api-use-default-includes=false&json-api-version=1.0"
        );

        let mut next_url = Some(url);
        let mut list: Vec<Comment> = vec![];
        while let Some(url) = next_url {
            let document: Document<Vec<Comment>> = self.fetch(&url).await?;

            list.extend(document.data);
            next_url = document
                .links
                .and_then(|links| links.next.map(|v| v.href.to_string()));
        }
        Ok(list)
    }
}
