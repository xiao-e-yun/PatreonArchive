use std::path::PathBuf;

use jsonapi_deserialize::{deserialize_document, Document, JsonApiDeserialize};
use log::{debug, log_enabled, trace};
use reqwest::header;
use reqwest_middleware::RequestBuilder;
use serde_json::Value;

use crate::{
    config::Config,
    patreon::{post::Post, Member, User},
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

        debug!("GET {}", url);
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
        let url = format!("https://www.patreon.com/api/members?include=campaign&fields[campaign]=is_active%2Cname%2Curl&filter[user_id]={}&filter[membership_type]=active_patron%2Cdeclined_patron%2Cfree_trial%2Cgifted_c2f%2Cgifted_f2f%2Cfree_member&fields[member]=is_free_member%2Ccampaign_pledge_amount_cents%2Ccampaign_currency&page[offset]=0&page[count]=1000&json-api-version=1.0&json-api-use-default-includes=false", user.id);

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
            "https://www.patreon.com/api/posts?include=attachments_media%2Cimages.null%2Caudio.null&fields%5Bpost%5D=comment_count%2Ccontent%2Ccurrent_user_can_view%2Cmin_cents_pledged_to_view%2Cembed%2Cimage%2Cpost_metadata%2Cpublished_at%2Cpost_type%2Ctitle%2Curl&fields%5Buser%5D=image_url%2Cfull_name%2Curl&fields%5Bmedia%5D=id%2Cimage_urls%2Cdownload_url%2Cmetadata%2Cfile_name&sort=-published_at&filter%5Bis_draft%5D=false&filter%5Baccessible_by_user_id%5D={}&filter%5Bcontains_exclusive_posts%5D=true&json-api-use-default-includes=false&json-api-version=1.0&filter%5Bcampaign_id%5D={}",
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
}
