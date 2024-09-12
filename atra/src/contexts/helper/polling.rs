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

use std::error::Error;
use smallvec::SmallVec;
use crate::contexts::traits::{SupportsConfigs, SupportsUrlGuarding, SupportsLinkState, SupportsUrlQueue, SupportsPolling};
use crate::link_state::{LinkState, LinkStateType};
use crate::queue::polling::{AbortCause, QueueExtractionError, UrlQueuePollResult};
use crate::runtime::ShutdownReceiver;
use crate::url::guard::{GuardianError, UrlGuardian};
use crate::url::queue::{UrlQueue, UrlQueueElement};
use crate::url::{AtraOriginProvider, UrlWithGuard};



impl<C> SupportsPolling for C where C: SupportsUrlQueue + SupportsConfigs + SupportsUrlGuarding + SupportsLinkState {
    type Guardian = C::Guardian;

    type Error = <C as SupportsLinkState>::Error;

    async fn poll_next_free_url<'a>(&'a self, shutdown_handle: impl ShutdownReceiver, max_miss: Option<u64>) -> UrlQueuePollResult<UrlWithGuard<'a, Self::Guardian>, Self::Error> {
        if self.url_queue().is_empty().await {
            UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty)
        } else {
            const MISSED_KEEPER_CACHE: usize = 8;

            let max_age = self.configs().crawl().max_queue_age;
            let manager = self.get_guardian();
            let mut missed_hosts = 0;
            let mut missed_host_cache = SmallVec::<[UrlQueueElement; MISSED_KEEPER_CACHE]>::new();
            let mut retries = self.url_queue().len().await;
            loop {
                if shutdown_handle.is_shutdown(){
                    if !missed_host_cache.is_empty() {
                        match self.url_queue().enqueue_all(missed_host_cache).await {
                            Err(err) => break UrlQueuePollResult::Err(
                                QueueExtractionError::QueueError(err)
                            ),
                            _ => {}
                        }
                    }
                    break UrlQueuePollResult::Abort(AbortCause::Shutdown);
                }
                match self.url_queue().dequeue().await {
                    Ok(Some(entry)) => {
                        retries = retries.saturating_sub(1);
                        if max_age != 0 && entry.age > max_age {
                            log::debug!("Drop {:?} from queue due to age.", entry);
                            continue;
                        }
                        match self.get_link_state(&entry.target).await {
                            Ok(found) => {
                                if let Some(found) = found {
                                    if drop_from_queue(self, &entry, &found).await {
                                        missed_hosts += 1;
                                        log::debug!("Drop {:?} from queue.", entry);
                                        continue;
                                    }
                                    if !found.typ.is_discovered() {
                                        missed_host_cache.push(entry);
                                        missed_hosts += 1;
                                        match push_logic_1(
                                            self,
                                            missed_hosts,
                                            missed_host_cache,
                                            &max_miss,
                                            retries
                                        ).await {
                                            UrlQueuePollResult::Ok(cache) => {
                                                missed_host_cache = cache;
                                                continue;
                                            }
                                            UrlQueuePollResult::Abort(cause) => {
                                                break UrlQueuePollResult::Abort(cause);
                                            }
                                            UrlQueuePollResult::Err(err) => {
                                                break UrlQueuePollResult::Err(err);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                break UrlQueuePollResult::Err(QueueExtractionError::LinkState(err));
                            }
                        }
                        match manager.try_reserve(&entry.target).await {
                            Ok(guard) => {
                                if !missed_host_cache.is_empty() {
                                    match self.url_queue().enqueue_all(missed_host_cache).await {
                                        Err(err) => break UrlQueuePollResult::Err(
                                            QueueExtractionError::QueueError(err)
                                        ),
                                        _ => {}
                                    }
                                }
                                break UrlQueuePollResult::Ok(
                                    unsafe{ UrlWithGuard::new_unchecked(guard, entry.target)}
                                );
                            }
                            Err(GuardianError::NoOriginError(_)) => {
                                break match self.url_queue().enqueue_all(missed_host_cache).await {
                                    Ok(_) => UrlQueuePollResult::Abort(AbortCause::NoHost(entry)),
                                    Err(err) => UrlQueuePollResult::Err(
                                        QueueExtractionError::QueueError(err)
                                    )
                                };
                            }
                            Err(GuardianError::AlreadyOccupied(_)) => {
                                missed_host_cache.push(entry);
                                missed_hosts += 1;
                                match push_logic_2(
                                    self,
                                    missed_hosts,
                                    missed_host_cache,
                                    &max_miss
                                ).await {
                                    UrlQueuePollResult::Ok(cache) => {
                                        missed_host_cache = cache;
                                        continue;
                                    }
                                    UrlQueuePollResult::Abort(cause) => {
                                        break UrlQueuePollResult::Abort(cause);
                                    }
                                    UrlQueuePollResult::Err(err) => {
                                        break UrlQueuePollResult::Err(err);
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        break UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty);
                    }
                    Err(err) => {
                        break UrlQueuePollResult::Err(QueueExtractionError::QueueError(err));
                    }
                }
            }
        }
    }
}

async fn drop_from_queue<C: SupportsConfigs>(context: &C, entry: &UrlQueueElement, state: &LinkState) -> bool {
    match state.typ {
        LinkStateType::Discovered => {false}
        LinkStateType::ProcessedAndStored => {
            let budget = if let Some(origin) = entry.target.atra_origin() {
                context.configs().crawl.budget.get_budget_for(&origin)
            } else {
                &context.configs().crawl.budget.default
            };
            budget.get_recrawl_interval().is_none()
        }
        LinkStateType::InternalError | LinkStateType::Unset | LinkStateType::Crawled | LinkStateType::ReservedForCrawl => {
            true
        }
        LinkStateType::Unknown(id) => {
            log::debug!("Some unknown link state of type {id} was found!");
            true
        }
    }
}


/// Some private push logic for the macro retrieve_seed, does also check if the retries fail.
async fn push_logic_1<C: SupportsUrlQueue, T: PartialOrd, E: std::error::Error, const N: usize>(
    context: &C,
    missed_hosts: T,
    missed_host_cache: SmallVec::<[UrlQueueElement; N]>,
    max_miss: &Option<T>,
    retries: usize,
) -> UrlQueuePollResult<SmallVec::<[UrlQueueElement; N]>, E> {
    if retries == 0 {
        match context.url_queue().enqueue_all(missed_host_cache).await {
            Ok(_) => UrlQueuePollResult::Abort(AbortCause::OutOfPullRetries),
            Err(err) => UrlQueuePollResult::Err(QueueExtractionError::QueueError(err))
        }
    } else {
        push_logic_2(
            context,
            missed_hosts,
            missed_host_cache,
            max_miss
        ).await
    }
}

/// Some private push logic for the macro retrieve_seed, but does not check for retries
async fn push_logic_2<C: SupportsUrlQueue, T: PartialOrd, E: std::error::Error, const N: usize>(
    context: &C,
    missed_hosts: T,
    missed_host_cache: SmallVec::<[UrlQueueElement; N]>,
    max_miss: &Option<T>,
) -> UrlQueuePollResult<SmallVec::<[UrlQueueElement; N]>, E> {
    if let Some(unpacked) = max_miss {
        if missed_hosts.gt(unpacked) {
            return match context.url_queue().enqueue_all(missed_host_cache).await {
                Ok(_) => UrlQueuePollResult::Abort(AbortCause::TooManyMisses),
                Err(err) => UrlQueuePollResult::Err(QueueExtractionError::QueueError(err))
            };
        }
    }
    if missed_host_cache.len() == N {
        return match context.url_queue().enqueue_all(missed_host_cache).await {
            Err(err) => UrlQueuePollResult::Err(QueueExtractionError::QueueError(err)),
            _ => UrlQueuePollResult::Ok(SmallVec::new())
        }
    }
    UrlQueuePollResult::Ok(missed_host_cache)
}