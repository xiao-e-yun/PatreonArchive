use std::path::PathBuf;

use log::{error, info};
use reqwest::header;
use reqwest_middleware::RequestBuilder;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    config::Config,
    fanbox::{Creator, FollowingCreator, Post, PostListItem, SupportingCreator},
};

use super::ArchiveClient;

pub type APIPost = Post;
pub type APIListCreatorPost = Vec<PostListItem>;
pub type APIListSupportingCreator = Vec<SupportingCreator>;
pub type APIListFollowingCreator = Vec<FollowingCreator>;
pub type APIListCreatorPaginate = Vec<String>;

#[derive(Debug, Clone)]
pub struct FanboxClient {
    inner: ArchiveClient,
    session: String,
    overwrite: bool,
}

impl FanboxClient {
    pub fn new(config: &Config) -> Self {
        let inner = ArchiveClient::new(config);
        let session = config.session();
        let overwrite = config.overwrite();
        Self { inner, session, overwrite }
    }

    fn wrap_request(&self, builder: RequestBuilder) -> RequestBuilder {
        const USER_AGENT: &str =
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 Edg/126.0.0.0";
        builder
            .header(header::COOKIE, &self.session)
            .header(header::ORIGIN, "https://www.fanbox.cc")
            .header(header::USER_AGENT, USER_AGENT)
    }

    pub async fn fetch<T: DeserializeOwned>(&self, url: &str) -> Result<T, FanboxAPIResponseError> {
        let (client, _semaphore) = self.inner.client().await;
        let request = client.get(url);
        let request = self.wrap_request(request);
        let response = request.send().await.expect("Failed to send request");
        let response = response.bytes().await.expect("Failed to get response body");

        match serde_json::from_slice::<FanboxAPIResponse<T>>(&response) {
            Ok(value) => Ok(value.body),
            Err(error) => {
                // try to parse as error
                match serde_json::from_slice::<FanboxAPIResponseError>(&response) {
                    Ok(response) => {
                        if response.error == "general_error" {
                            error!("The session is invalid or expired");
                            error!("Or the API has changed");
                        }
                        Err(response)
                    }
                    Err(_) => panic!("{:?}\n{}", error, String::from_utf8_lossy(&response)),
                }
            }
        }
    }

    pub async fn download(&self, url: &str, path: PathBuf) -> Result<(), reqwest::Error> {
        let (client, _semaphore) = self.inner.client().await;
        let request = client.get(url);
        let request = self.wrap_request(request);
        let response = request.send().await.expect("Failed to send request");

        let skip = !self.overwrite && path.exists();
        if skip {
            info!("Download was skip ({})", path.display());
        } else {
            let mut file = tokio::fs::File::create(path).await.unwrap();
            self.inner
                .download(response, &mut file)
                .await?;
        }

        Ok(())
    }

    pub async fn get_supporting_creators(
        &self,
    ) -> Result<APIListSupportingCreator, Box<dyn std::error::Error>> {
        let url = "https://api.fanbox.cc/plan.listSupporting";
        let list: APIListSupportingCreator = self
            .fetch(url)
            .await
            .expect("Failed to get supporting authors");
        Ok(list)
    }

    pub async fn get_following_creators(
        &self,
    ) -> Result<APIListFollowingCreator, Box<dyn std::error::Error>> {
        let url = "https://api.fanbox.cc/creator.listFollowing";
        let list: APIListFollowingCreator = self
            .fetch(url)
            .await
            .expect("Failed to get following authors");
        Ok(list)
    }

    pub async fn get_posts(
        &self,
        creator: &Creator,
    ) -> Result<APIListCreatorPost, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.fanbox.cc/post.paginateCreator?creatorId={}",
            creator.id()
        );
        let urls: APIListCreatorPaginate = self.fetch(&url).await.expect("Failed to get post list");

        let mut tasks = Vec::new();
        for url in urls {
            let client = self.clone();
            let future = async move {
                client
                    .fetch::<APIListCreatorPost>(&url)
                    .await
                    .expect("Failed to get post")
            };
            tasks.push(tokio::spawn(future));
        }

        let mut posts = Vec::new();
        for task in tasks {
            posts.extend(task.await?.into_iter());
        }

        Ok(posts)
    }

    pub async fn get_post(&self, post_id: String) -> Result<APIPost, Box<dyn std::error::Error>> {
        let url = format!("https://api.fanbox.cc/post.info?postId={}", post_id);
        let post: APIPost = self.fetch(&url).await.expect("Failed to get post");
        Ok(post)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct FanboxAPIResponse<T> {
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct FanboxAPIResponseError {
    error: String,
}
