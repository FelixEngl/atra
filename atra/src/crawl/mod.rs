// Copyright 2024 Felix Engl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::error::Error;
use std::io;
use std::sync::Arc;

use strum::{Display, EnumString};
use tokio::task::yield_now;

pub use crawler::result::{CrawlResult};
pub use crawler::slim::*;
pub use crawler::*;

use crate::contexts::traits::{
    SupportsCrawlResults, SupportsLinkSeeding, SupportsLinkState, SupportsPolling,
    SupportsSlimCrawlResults,
};
use crate::contexts::Context;
use crate::queue::polling::{AbortCause, QueueExtractionError, UrlQueuePollResult};
use crate::queue::QueueError;
use crate::runtime::ShutdownReceiver;
use crate::sync::barrier::{ContinueOrStop, WorkerBarrier};
use crate::url::guard::GuardianError;

#[cfg(test)]
pub use crawler::result::test;

mod crawler;
pub mod db;

/// The exit state of the crawl task
#[derive(Debug, Copy, Clone, Eq, PartialEq, EnumString, Display)]
pub enum ExitState {
    Shutdown,
    NoMoreElements,
}

/// A consumer for some kind of error. Allows to return an error if necessary to stop the crawling.
pub trait ErrorConsumer<E>: Send + Sync {
    type Error;
    fn consume_crawl_error(&self, e: E) -> Result<(), Self::Error>;
    fn consume_poll_error(&self, e: E) -> Result<(), Self::Error>;
}

/// The core method for crawling data.
pub async fn crawl<C, S, E, EC>(
    context: C,
    shutdown: S,
    worker_barrier: Arc<WorkerBarrier>,
    consumer: EC,
) -> Result<ExitState, EC::Error>
where
    C: Context,
    S: ShutdownReceiver,
    E: From<<C as SupportsSlimCrawlResults>::Error>
        + From<<C as SupportsLinkSeeding>::Error>
        + From<<C as SupportsCrawlResults>::Error>
        + From<<C as SupportsLinkState>::Error>
        + From<<C as SupportsPolling>::Error>
        + From<crate::client::ClientError>
        + From<QueueError>
        + From<io::Error>
        + Error,
    EC: ErrorConsumer<E> + Send + Sync,
{
    const PATIENCE: i32 = 150;

    let mut patience = PATIENCE;

    loop {
        if shutdown.is_shutdown() || worker_barrier.is_cancelled() {
            if let ContinueOrStop::Cancelled(value) = worker_barrier
                .wait_for_is_cancelled(&context, Ok(ExitState::Shutdown))
                .await
            {
                return value;
            }
        }

        // todo: keep all alive as long as there is the possebility to encounter a new url with a different url.
        let provider = context
            .poll_next_free_url(shutdown.weak_handle(), None)
            .await;

        // with_seed_provider_context! {let provider = from context.as_ref();}
        match provider {
            UrlQueuePollResult::Ok(guard) => {
                if patience != PATIENCE {
                    patience = PATIENCE;
                }

                let guarded_seed = guard.get_unguarded_seed();

                let mut crawler = WebsiteCrawlerBuilder::new(context.configs().crawl())
                    .build(guarded_seed)
                    .await;

                crawler.crawl(&context, shutdown.clone(), &consumer).await?
            }
            UrlQueuePollResult::Abort(cause) => {
                if patience < 0 {
                    patience = PATIENCE;
                    if let ContinueOrStop::Cancelled(value) = worker_barrier
                        .wait_for_is_cancelled(&context, Ok(ExitState::NoMoreElements))
                        .await
                    {
                        return value;
                    }
                } else {
                    match cause {
                        AbortCause::TooManyMisses => {
                            patience -= 2;
                        }
                        AbortCause::OutOfPullRetries => {
                            patience -= 5;
                        }
                        AbortCause::QueueIsEmpty => {
                            patience -= 10;
                        }
                        AbortCause::NoHost(dropped) => {
                            log::warn!("Drop {} from queue due to NoDomain error.", dropped.target)
                        }
                        AbortCause::Shutdown => {
                            log::debug!("Shutdown while searching queue.");
                            continue;
                        }
                    }
                    yield_now().await;
                    continue;
                }
            }
            UrlQueuePollResult::Err(err) => {
                match err {
                    QueueExtractionError::HostManager(err) => match err {
                        GuardianError::NoOriginError(url) => {
                            log::error!("The url {url} does not result in a domain.")
                        }
                        GuardianError::AlreadyOccupied(info) => {
                            log::debug!("The domain {info:?} is already occupied.")
                        }
                    },
                    QueueExtractionError::LinkState(err) => {
                        consumer.consume_poll_error(err.into())?;
                    }
                    QueueExtractionError::QueueError(err) => {
                        consumer.consume_poll_error(err.into())?;
                    }
                }
                continue;
            }
        }
    }
}
