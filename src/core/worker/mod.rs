//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::sync::Arc;
use strum::{Display, EnumString};
use tokio::task::yield_now;
use crate::core::contexts::Context;
use crate::core::crawl::website_crawler::{WebsiteCrawlerBuilder};
use crate::core::database_error::DatabaseError;
use crate::core::link_state::LinkStateDBError;
use crate::core::origin::errors::OriginManagerError;
use crate::core::seed_provider::{AbortCause, get_seed_from_context, QueueExtractionError, RetrieveProviderResult};
use crate::core::shutdown::{ShutdownReceiver};
use crate::core::sync::barrier::{ContinueOrStop, SupportsWorkerId, WorkerBarrier};

/// The exit state of the crawl task
#[derive(Debug, Copy, Clone, Eq, PartialEq, EnumString, Display)]
pub enum ExitState {
    Shutdown,
    NoMoreElements
}



/// The core method for crawling data.
pub async fn work<C, S>(context: C, shutdown: S, worker_barrier: Arc<WorkerBarrier>) -> Result<ExitState, ()>
    where C: Context + SupportsWorkerId,
          S: ShutdownReceiver
{
    const PATIENCE: i32 = 150;

    let mut patience = PATIENCE;

    loop {

        if shutdown.is_shutdown() || worker_barrier.is_cancelled() {
            if let ContinueOrStop::Cancelled(value) = worker_barrier.wait_for_is_cancelled(&context, Ok(ExitState::Shutdown)).await {
                return value
            }
        }


        // todo: keep all alive as long as there is the possebility to encounter a new url with a different url.
        let provider = get_seed_from_context(&context, shutdown.weak_handle(), None).await;

        // with_seed_provider_context! {let provider = from context.as_ref();}
        match provider {
            RetrieveProviderResult::Ok(guard) => {
                if patience != PATIENCE {
                    patience = PATIENCE;
                }

                let guarded_seed = guard.get_seed();

                let mut crawler =
                    WebsiteCrawlerBuilder::new(context.configs().crawl()).build(guarded_seed).await;

                match crawler.crawl(&context, shutdown.clone()).await {
                    Ok(_) => {}
                    Err(errors) => {
                        for error in errors {
                            log::error!("{}", error)
                        }
                    }
                }
            }
            RetrieveProviderResult::Abort(cause) => {
                if patience < 0 {
                    patience = PATIENCE;
                    if let ContinueOrStop::Cancelled(value) = worker_barrier.wait_for_is_cancelled(&context, Ok(ExitState::NoMoreElements)).await {
                        return value
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
                            continue
                        }
                    }
                    yield_now().await;
                    continue;
                }
            }
            RetrieveProviderResult::Err(err) => {
                match err {
                    QueueExtractionError::HostManager(err) => {
                        match err {
                            OriginManagerError::NoOriginError(url) => {
                                log::error!("The url {url} does not result in a domain.")
                            }
                            OriginManagerError::AlreadyOccupied(info) => {
                                log::info!("The domain {info:?} is already occupied.")
                            }
                        }
                    }
                    QueueExtractionError::LinkState(err) => {
                        match err {
                            LinkStateDBError::Database(err) => {
                                match err {
                                    err @ DatabaseError::RecoverableFailure { .. } => {
                                        log::error!("Failed a recoverable situation multiple times, continue work! {err}")
                                    }
                                    others => {
                                        log::error!("Unhandled: {}", others)
                                    }
                                }
                            }
                            LinkStateDBError::LinkStateError(err) => {
                                log::error!("{}", err)
                            }
                        }
                    }
                    QueueExtractionError::QueueError(err) => {
                        log::error!("{}", err)
                    }
                }

                continue;
            }
        }
    }
}


#[cfg(test)]
mod test_sync {
    use itertools::Itertools;

    #[test]
    fn test_fuzzy(){
        const EXAMPLE: [[u8; 5]; 3] = [
            [1,2,3,4,5],
            [4,2,3,2,5],
            [1,2,3,47,0],
        ];

        const EXAMPLE2: [[u8; 5]; 3] = [
            [1,2,3,4,5],
            [4,2,3,2,5],
            [1,2,3,47,1],
        ];

        let x = EXAMPLE.to_vec().iter().flatten().cloned().collect_vec();
        let a = fuzzyhash::FuzzyHash::new(x);
        let mut b = fuzzyhash::FuzzyHash::default();
        for a in &EXAMPLE2 {
            b.update(a)
        }
        b.finalize();

        println!("{:?}", a.compare_to(&b));
        println!("{} {}", a.to_string(), b.to_string());
    }
}