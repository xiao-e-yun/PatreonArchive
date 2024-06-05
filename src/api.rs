use std::{collections::HashMap, future::Future, path::PathBuf, sync::Arc};

use reqwest::{Client, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    sync::{Semaphore, SemaphorePermit},
};
use url::Url;

use crate::{
    author::{Author, FollowingAuthor, SupportingAuthor},
    config::Config,
    post::{Post, PostList, PostListCache},
    utils::Request,
};

const RETRY_LIMIT: u32 = 3;

#[derive(Debug, Clone)]
pub struct ArchiveClientInner {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl ArchiveClientInner {
    fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            semaphore: Arc::new(Semaphore::new(config.limit())),
        }
    }
    fn client(&self) -> impl Future<Output = (ClientWithMiddleware, SemaphorePermit)> + Send
    where
        Self: Sync,
    {
        async {
            let semaphore = self.semaphore.acquire().await.unwrap();
            let retry_policy = ExponentialBackoff::builder().build_with_max_retries(RETRY_LIMIT);
            let client = ClientBuilder::new(self.client.clone())
                .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                .build();
            (client, semaphore)
        }
    }
}

pub trait ArchiveClient {
    type ResponseError: DeserializeOwned;
    fn new(config: Config) -> Self;
    fn inner(&self) -> &ArchiveClientInner;
    fn inner_mut(&mut self) -> &mut ArchiveClientInner;
    fn cookies(&self) -> Vec<String>;
    fn builder(&self, builder: RequestBuilder) -> RequestBuilder;

    fn client(&self) -> impl Future<Output = (ClientWithMiddleware, SemaphorePermit)> + Send
    where
        Self: Sync,
    {
        self.inner().client()
    }

    fn build_request(&self, requset: RequestBuilder) -> RequestBuilder {
        let cookies = self.cookies().join(";");
        self.builder(requset.header("Cookie", cookies))
    }

    fn _get(&self, url: Url) -> impl Future<Output = Response> + Send
    where
        Self: Sync,
    {
        async move {
            let (client, _) = self.client().await;
            let builder = client.get(url.clone());
            let builder = self
                .builder(builder)
                .header("Cookie", self.cookies().join(";"));

            builder.send().await.unwrap()
        }
    }
    fn _download(&self, url: Url, path: PathBuf) -> impl Future<Output = ()> + Send
    where
        Self: Sync,
    {
        async move {
            let response = self._get(url).await;
            let stream = response.bytes().await.unwrap();
            let mut file = File::create(&path).await.unwrap();
            file.write(&stream).await.unwrap();
        }
    }
    fn _get_json<T: DeserializeOwned>(
        &self,
        url: Url,
    ) -> impl Future<Output = Result<T, Self::ResponseError>> + Send
    where
        Self: Sync,
    {
        async {
            let response = self._get(url).await;
            let bytes = response.bytes().await.unwrap();
            match serde_json::from_slice(&bytes) {
                Ok(value) => Ok(value),
                Err(e) => {
                    let Ok(response) = serde_json::from_slice(&bytes) else {
                        panic!("{:?}", e)
                    };
                    Err(response)
                }
            }
        }
    }
}

//==============================================================================
//
//==============================================================================
#[derive(Debug, Clone)]
pub struct FanboxClient {
    inner: ArchiveClientInner,
    session: String,
}

impl ArchiveClient for FanboxClient {
    type ResponseError = APIResponseError;
    fn new(config: Config) -> Self {
        Self {
            inner: ArchiveClientInner::new(&config),
            session: config.session(),
        }
    }
    fn inner(&self) -> &ArchiveClientInner {
        &self.inner
    }
    fn inner_mut(&mut self) -> &mut ArchiveClientInner {
        &mut self.inner
    }

    fn cookies(&self) -> Vec<String> {
        vec![self.session.clone()]
    }

    fn builder(&self, builder: RequestBuilder) -> RequestBuilder {
        builder.header("Origin", "https://www.fanbox.cc")
    }
}

impl FanboxClient {
    pub async fn get_post(&self, post_id: u32) -> Post {
        let url = Url::parse(&format!(
            "https://api.fanbox.cc/post.info?postId={}",
            post_id
        ))
        .unwrap();
        let response: APIPost = Self::panic_error(self._get_json(url).await);
        response.raw()
    }

    pub async fn get_post_list(
        &self,
        author: Author,
        skip_free: bool,
        cache: Option<Arc<PostListCache>>,
    ) -> (Vec<u32>, PostListCache) {
        let mut next_url = Some(
            Url::parse(&format!(
                "https://api.fanbox.cc/post.listCreator?creatorId={}&limit=300",
                author.id()
            ))
            .unwrap(),
        );

        let has_cache = cache.is_some();
        let cache = cache.unwrap_or_default();

        let mut result = Vec::new();
        let mut updated_cache = HashMap::new();

        while let Some(url) = next_url {
            let response = Self::panic_error(self._get_json::<APIListCreator>(url).await).raw();
            next_url = response.next_url.clone();
            result.extend(response.items.into_iter().filter_map(|f| {
                if f.fee_required > author.fee() || (skip_free && f.fee_required == 0) {
                    return None;
                }

                if has_cache {
                    let last_updated = cache.get(&f.id).cloned().unwrap_or_default();
                    if f.updated_datetime == last_updated {
                        return None;
                    }

                    updated_cache.insert(f.id, f.updated_datetime);
                }

                Some(f.id)
            }));
        }

        (result, updated_cache)
    }

    pub async fn get_supporting_authors(&self) -> Vec<SupportingAuthor> {
        let url = Url::parse("https://api.fanbox.cc/plan.listSupporting").unwrap();
        let response: APIListSupporting = Self::panic_error(self._get_json(url).await);
        response.raw()
    }

    pub async fn get_following_authors(&self) -> Vec<FollowingAuthor> {
        let url = Url::parse("https://api.fanbox.cc/creator.listFollowing").unwrap();
        let response: APIListFollowing = Self::panic_error(self._get_json(url).await);
        response.raw()
    }

    pub async fn download(&self, url: Url, path: PathBuf) {
        self._download(url, path).await;
    }

    fn panic_error<T>(response: Result<T, APIResponseError>) -> T {
        match response {
            Ok(value) => value,
            Err(APIResponseError { error }) => panic!("{} (tips: check your session)", error),
        }
    }
}

pub type APIPost = Request<Post>;
pub type APIListCreator = Request<PostList>;
pub type APIListSupporting = Request<Vec<SupportingAuthor>>;
pub type APIListFollowing = Request<Vec<FollowingAuthor>>;

#[derive(Deserialize, Serialize, Debug, Clone, Hash)]
pub struct APIResponseError {
    error: String,
}
