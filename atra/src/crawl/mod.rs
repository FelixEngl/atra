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

pub use crawler::result::CrawlResult;
pub use crawler::slim::*;
pub use crawler::*;

use crate::contexts::traits::{
    SupportsCrawlResults, SupportsCrawling, SupportsLinkSeeding, SupportsLinkState,
    SupportsPolling, SupportsSlimCrawlResults,
};
use crate::contexts::Context;
use crate::queue::QueueError;
use crate::queue::{AbortCause, QueueExtractionError, UrlQueuePollResult};
use crate::runtime::ShutdownReceiver;
use crate::sync::{ContinueOrStop, WorkerBarrier};

use crate::link_state::LinkStateManager;
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

unsafe impl Send for ExitState{}
unsafe impl Sync for ExitState{}

/// A consumer for some kind of error. Allows to return an error if necessary to stop the crawling.
pub trait ErrorConsumer<E> {
    type Error;
    fn consume_init_error(&self, e: E) -> Result<(), Self::Error>;
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
        + From<<<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>
        + From<<C as SupportsPolling>::Error>
        + From<<C as SupportsCrawling>::Error>
        + From<QueueError>
        + From<io::Error>
        + Error,
    EC: ErrorConsumer<E>,
{
    const PATIENCE: i32 = 150;

    let mut patience = PATIENCE;

    loop {
        if shutdown.is_shutdown() || worker_barrier.is_cancelled() {
            if let ContinueOrStop::Cancelled(value) = worker_barrier
                .wait_for_is_cancelled(&context, Ok(ExitState::Shutdown))
                .await
            {
                log::info!("Worker task stopping due to.");
                return value;
            }
        }

        // todo: keep all alive as long as there is the possebility to encounter a new url with a different url.
        let provider = context.poll_next_free_url(shutdown.clone(), None).await;

        // with_seed_provider_context! {let provider = from context.as_ref();}
        match provider {
            UrlQueuePollResult::Ok(guard) => {
                if patience != PATIENCE {
                    patience = PATIENCE;
                }

                match context.create_crawl_task(guard.get_guarded_seed()) {
                    Ok(mut task) => task.run(&context, shutdown.clone(), &consumer).await?,
                    Err(err) => {
                        consumer.consume_crawl_error(err.into())?;
                    }
                }
            }
            UrlQueuePollResult::Abort(cause) => {
                if patience < 0 {
                    patience = PATIENCE;
                    if let ContinueOrStop::Cancelled(value) = worker_barrier
                        .wait_for_is_cancelled(&context, Ok(ExitState::NoMoreElements))
                        .await
                    {
                        log::debug!("Shutting down worker due to patience!");
                        return value;
                    }
                } else {
                    match cause {
                        AbortCause::TooManyMisses => {
                            patience -= 2;
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
                    // QueueExtractionError::GuardianError(err) => match err {
                    //     GuardianError::NoOriginError(url) => {
                    //         log::error!("The url {url} does not result in a domain!")
                    //     }
                    //     GuardianError::AlreadyOccupied(info) => {
                    //         log::debug!("The domain {info:?} is already in use.")
                    //     }
                    // },
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
