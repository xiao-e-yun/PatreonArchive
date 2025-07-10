use std::path::PathBuf;

use jsonapi_deserialize::{deserialize_document, Document, JsonApiDeserialize};
use log::{debug, log_enabled, trace};
use reqwest::header;
use reqwest_middleware::RequestBuilder;
use serde_json::Value;

use crate::{
    config::Config,
    patreon::{comment::Comment, post::Post, Member, User},
};

use super::ArchiveClient;

#[derive(Debug, Clone)]
pub struct PatreonClient {
    inner: ArchiveClient,
    session: String,
    overwrite: bool,
}

impl PatreonClient {
    pub fn new(config: &Config) -> Self {
        let inner = ArchiveClient::new(config);
        let session = config.session();
        let overwrite = config.overwrite();
        Self {
            inner,
            session,
            overwrite,
        }
    }

    fn wrap_request(&self, builder: RequestBuilder) -> RequestBuilder {
        const USER_AGENT: &str =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
        builder
            .header(header::COOKIE, &self.session)
            .header(header::ORIGIN, "https://www.patreon.com")
            .header(header::USER_AGENT, USER_AGENT)
    }

    pub async fn fetch<T: JsonApiDeserialize>(
        &self,
        url: &str,
    ) -> Result<Document<T>, jsonapi_deserialize::Error> {
        let (client, _semaphore) = self.inner.client().await;
        let request = client.get(url);
        let request = self.wrap_request(request);
        let response = request.send().await.expect("Failed to send request");
        let response = response.text().await.expect("Failed to get response body");

        debug!("GET {url}");
        if log_enabled!(log::Level::Trace) {
            let response: Value = serde_json::from_str(&response).unwrap();
            trace!("{}", serde_json::to_string_pretty(&response).unwrap());
        }

        deserialize_document(&response)
    }

    pub async fn download(&self, url: &str, path: PathBuf) -> Result<(), reqwest::Error> {
        if !self.overwrite && path.exists() {
            debug!("Download was skip ({})", path.display());
            return Ok(());
        }

        let (client, _semaphore) = self.inner.client().await;
        let request = client.get(url);
        let request = self.wrap_request(request);
        let response = request.send().await.expect("Failed to send request");

        debug!("Downloading {} to {}", url, path.display());
        let mut file = tokio::fs::File::create(path).await.unwrap();
        self.inner.download(response, &mut file).await?;

        Ok(())
    }

    pub async fn get_current_user_id(&self) -> Result<User, Box<dyn std::error::Error>> {
        let url = "https://www.patreon.com/api/current_user?include=[]&fields[user]=id,full_name";
        let list: Document<User> = self.fetch(url).await?;

        Ok(list.data)
    }

    pub async fn get_members(
        &self,
        user: &User,
    ) -> Result<Vec<Member>, Box<dyn std::error::Error>> {
        let url = format!("https://www.patreon.com/api/members?include=campaign&fields[campaign]=is_active,name,url&filter[user_id]={}&filter[membership_type]=active_patron,declined_patron,free_trial,gifted_c2f,gifted_f2f&fields[member]=campaign_pledge_amount_cents,campaign_currency&page[offset]=0&page[count]=1000&json-api-version=1.0&json-api-use-default-includes=false", user.id);

        let mut next_url = Some(url);
        let mut list: Vec<Member> = vec![];
        while let Some(url) = next_url {
            let document: Document<Vec<Member>> = self.fetch(&url).await?;

            list.extend(document.data);
            next_url = document.links.unwrap().next.map(|v| v.href.to_string())
        }

        Ok(list)
    }

    pub async fn get_posts(
        &self,
        user: &User,
        campaign: &str,
    ) -> Result<Vec<Post>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://www.patreon.com/api/posts?include=media,audio.null,audio_preview.null,poll.null,poll.choices,content_unlock_options.reward,user_defined_tags&fields[post]=comment_count,content,current_user_can_view,embed,image,post_metadata,published_at,post_type,title,url&fields[user]=image_url,full_name,url&fields[media]=id,image_urls,download_url,metadata,file_name&sort=-published_at&filter[is_draft]=false&filter[accessible_by_user_id]={}&filter[contains_exclusive_posts]=true&json-api-use-default-includes=false&json-api-version=1.0&filter[campaign_id]={}",
            user.id,
            campaign
        );

        let mut next_url = Some(url);
        let mut list: Vec<Post> = vec![];
        while let Some(url) = next_url {
            let document: Document<Vec<Post>> = self.fetch(&url).await?;

            list.extend(document.data);
            next_url = document
                .links
                .and_then(|links| links.next.map(|v| v.href.to_string()));
        }

        Ok(list)
    }

    pub async fn get_comments(
        &self,
        post_id: &str,
    ) -> Result<Vec<Comment>, Box<dyn std::error::Error>> {
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
