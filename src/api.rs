use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
};

use indicatif::ProgressBar;
use jsonapi_deserialize::{deserialize_document, Document, JsonApiDeserialize};
use log::{debug, trace};
use post_archiver_utils::{ArchiveClient, Limit, RequestError, Result};
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client,
};

use crate::{
    config::{Config, IncOnDrop},
    patreon::{comment::Comment, post::Post, Member, User},
};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";

/// Fields requested for posts, shared by the listing and single-post endpoints.
const POST_QUERY: &str = "include=campaign,media,audio.null,audio_preview.null,poll.null,poll.choices,content_unlock_options.reward,user_defined_tags&fields[post]=comment_count,content,current_user_can_view,embed,image,post_metadata,published_at,post_type,title,url&fields[campaign]=name,url&fields[media]=id,image_urls,download_url,metadata,file_name&json-api-use-default-includes=false&json-api-version=1.0";

fn patreon_headers(config: &Config) -> HeaderMap {
    HeaderMap::from_iter([
        (header::COOKIE, config.session().parse().unwrap()),
        (header::USER_AGENT, HeaderValue::from_static(USER_AGENT)),
        (header::ORIGIN, HeaderValue::from_static("https://www.patreon.com")),
    ])
}

#[derive(Debug, Clone)]
pub struct PatreonClient {
    inner: ArchiveClient,
}

impl PatreonClient {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .default_headers(patreon_headers(config))
            .build()
            .unwrap();
        let limit = Limit {
            per_min: config.limit(),
            ..Default::default()
        };

        Self {
            inner: ArchiveClient::new(client, limit),
        }
    }

    /// Patreon speaks JSON:API, so fetch the raw body and let `jsonapi_deserialize`
    /// resolve `included` relationships.
    pub async fn fetch<T: JsonApiDeserialize>(&self, url: &str) -> Result<Document<T>> {
        let request = self.inner.client.get(url);
        let body = match self
            .inner
            .request(&request, |res| async move { Ok(res.text().await?) })
            .await
        {
            Ok(body) => body,
            Err(RequestError::UnexpectedStatus(status, body)) => {
                debug!("{status} response body: {body}");
                return Err(RequestError::UnexpectedStatus(status, body).into());
            }
            Err(e) => return Err(e.into()),
        };

        trace!("Fetched {url}");
        deserialize_document(&body)
            .map_err(|e| RequestError::InvalidResponse(e.to_string()).into())
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
            next_url = document
                .links
                .and_then(|links| links.next.map(|v| v.href.to_string()));
        }

        Ok(list)
    }

    pub fn get_posts_url(&self, user: &User, campaign: &str) -> String {
        format!(
            "https://www.patreon.com/api/posts?{POST_QUERY}&sort=-published_at&filter[is_draft]=false&filter[accessible_by_user_id]={}&filter[contains_exclusive_posts]=true&filter[campaign_id]={}",
            user.id, campaign
        )
    }

    pub async fn get_posts(&self, url: &str) -> Result<(Vec<Post>, Option<String>)> {
        let document: Document<Vec<Post>> = self.fetch(url).await?;

        let next_url = document
            .links
            .and_then(|links| links.next.map(|v| v.href.to_string()));

        Ok((document.data, next_url))
    }

    /// Fetch a single post. Media URLs are signed and expire, so retried posts
    /// must be fetched again instead of reusing URLs from an earlier run.
    pub async fn get_post(&self, post_id: &str) -> Result<Post> {
        let url = format!("https://www.patreon.com/api/posts/{post_id}?{POST_QUERY}");
        let document: Document<Post> = self.fetch(&url).await?;
        Ok(document.data)
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

/// Downloads media with its own rate limit, so file downloads don't eat into the API budget.
#[derive(Debug, Clone)]
pub struct PatreonDownloader {
    inner: ArchiveClient,
    pb: ProgressBar,
}

impl PatreonDownloader {
    pub fn new(config: &Config, pb: ProgressBar) -> Self {
        let client = Client::builder()
            .default_headers(patreon_headers(config))
            .build()
            .unwrap();
        let limit = Limit {
            per_min: NonZeroU32::new(90).unwrap(),
            ..Default::default()
        };

        Self {
            inner: ArchiveClient::new(client, limit),
            pb,
        }
    }

    pub async fn download(&self, with_in: &Path, url: &str) -> Result<PathBuf> {
        self.pb.inc_length(1);
        let _guard = IncOnDrop(self.pb.clone());
        let path = self.inner.download(with_in, url).await?;
        trace!("Downloaded {url}");
        Ok(path)
    }
}
