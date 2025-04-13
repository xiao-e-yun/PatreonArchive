pub mod patreon;

use futures::StreamExt;
use reqwest::{Client, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::sync::Arc;
use tokio::{
    fs::File,
    sync::{Semaphore, SemaphorePermit},
};

use crate::config::Config;

const RETRY_LIMIT: u32 = 3;

#[derive(Debug, Clone)]
pub struct ArchiveClient {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl ArchiveClient {
    fn new(config: &Config) -> Self {
        let permits = config.limit();
        Self {
            client: Client::new(),
            semaphore: Arc::new(Semaphore::new(permits)),
        }
    }
    async fn client(&self) -> (ClientWithMiddleware, SemaphorePermit) {
        let semaphore = self.semaphore.acquire().await.unwrap();
        let client = self.client_without_semaphore();
        (client, semaphore)
    }
    fn client_without_semaphore(&self) -> ClientWithMiddleware {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(RETRY_LIMIT);

        ClientBuilder::new(self.client.clone())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    }

    async fn download(&self, response: Response, file: &mut File) -> Result<(), reqwest::Error> {
        let mut stream = response.bytes_stream();
        while let Some(bytes) = stream.next().await {
            tokio::io::copy(&mut bytes?.as_ref(), file).await.unwrap();
        }
        Ok(())
    }
}
