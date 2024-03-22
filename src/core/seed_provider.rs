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

use smallvec::SmallVec;
use thiserror::Error;
use crate::core::contexts::{Context};
use crate::core::crawl::errors::SeedCreationError;
use crate::core::crawl::seed::{GuardedSeed, UnguardedSeed};
use crate::core::crawl::website_crawler::WebsiteCrawlerBuilderError;
use crate::core::domain::{DomainGuard, DomainManager, DomainManagerError};
use crate::core::link_state::{LinkState, LinkStateDBError, LinkStateType};
use crate::core::queue::QueueError;
use crate::core::shutdown::{ShutdownReceiver};
use crate::core::url::queue::{UrlQueue, UrlQueueElement};
use crate::core::UrlWithDepth;
use crate::core::link_state::LinkStateType::Discovered;

/// A guard with an associated seed url
pub struct GuardedSeedUrlProvider<'a, T: DomainManager> {
    guard: DomainGuard<'a, T>,
    seed_url: UrlWithDepth
}

impl<'a, T: DomainManager> GuardedSeedUrlProvider<'a, T> {

    /// Creates a DomainGuardWithSeed but asserts that the seed creation can wor beforehand.
    #[allow(dead_code)]
    pub fn new(guard: DomainGuard<'a, T>, seed_url: UrlWithDepth) -> Result<Self, SeedCreationError> {
        if let Some(domain) = seed_url.domain() {
            if guard.domain().eq(&domain) {
                Ok(unsafe{Self::new_unchecked(guard, seed_url)})
            } else {
                Err(SeedCreationError::GuardAndUrlDifferInDomain {
                    domain_from_url: domain.inner().clone(),
                    domain_from_guard: guard.domain().inner().clone()
                })
            }
        } else {
            Err(SeedCreationError::NoDomain)
        }
    }

    /// Creates a DomainGuardWithSeed without doing any domain checks.
    pub unsafe fn new_unchecked(guard: DomainGuard<'a, T>, seed_url: UrlWithDepth) -> Self {
        Self {
            guard,
            seed_url
        }
    }

    /// Returns the domain guard
    #[allow(dead_code)] pub fn guard(&self) -> &DomainGuard<'a, T> {
        &self.guard
    }

    /// Returns the seed url
    #[allow(dead_code)] pub fn seed_url(&self) -> &UrlWithDepth {
        &self.seed_url
    }

    /// Returns a guarded seed instance
    pub fn get_guarded_seed<'b>(&'b self) -> GuardedSeed<'b, 'a, T> {
        unsafe{GuardedSeed::new_unchecked(&self.guard, &self.seed_url)}
    }

    pub fn get_seed(&self) -> UnguardedSeed {
        unsafe {UnguardedSeed::new_unchecked(self.seed_url.clone(), self.guard.domain.clone())}
    }

    // /// Converts this to a tuple
    // #[allow(dead_code)] pub fn to_tuple(self) -> (DomainGuard<'a, T>, UrlWithDepth) {
    //     (self.guard, self.seed_url)
    // }
}


/// The result of the GuardedSeedUrlProvider extraction.
/// Helps to interpret what happened
pub enum RetrieveProviderResult<T> {
    Ok(T),
    Abort(AbortCause),
    Err(QueueExtractionError)
}


/// The abort cause for something. Can be used as error, but it can also be used for simple fallthrough.
#[derive(Debug, Error)]
pub enum AbortCause {
    #[error("The number of misses was higher than the maximum. Try again later.")]
    TooManyMisses,
    #[error("No valid domain for crawl found.")]
    OutOfPullRetries,
    #[error("The queue is empty.")]
    QueueIsEmpty,
    #[error("The element does not have a domain.")]
    NoDomain(UrlQueueElement),
    #[error("Shutdown")]
    Shutdown
}


/// All possible errors that can happen when retrieving a provider
#[derive(Debug, Error)]
pub enum QueueExtractionError {
    #[error(transparent)]
    DomainManager(#[from] DomainManagerError),
    #[error(transparent)]
    TaskBuilderFailed(#[from] WebsiteCrawlerBuilderError),
    #[error(transparent)]
    LinkState(#[from] LinkStateDBError),
    #[error(transparent)]
    QueueError(#[from] QueueError),
}


/// Creates with the given context and the max misses a guarded seed provider.
pub async fn get_seed_from_context<'a, C: Context>(context_ref: &'a C, shutdown_handle: impl ShutdownReceiver, max_miss: Option<u64>) -> RetrieveProviderResult<GuardedSeedUrlProvider<'a, C::DomainManager>> {
    if context_ref.url_queue().is_empty().await {
        return RetrieveProviderResult::Abort(AbortCause::QueueIsEmpty);
    } else {
        const MISSED_KEEPER_CACHE: usize = 8;




        let max_age = context_ref.configs().crawl().max_queue_age;
        let manager = context_ref.get_domain_manager();
        let mut missed_domains = 0;
        let mut missed_domains_cache = SmallVec::<[UrlQueueElement; MISSED_KEEPER_CACHE]>::new();
        let mut retries = context_ref.url_queue().len().await;
        return loop {
            if shutdown_handle.is_shutdown(){
                if !missed_domains_cache.is_empty() {
                    match context_ref.url_queue().enqueue_all(missed_domains_cache).await {
                        Err(err) => break RetrieveProviderResult::Err(
                            QueueExtractionError::QueueError(err)
                        ),
                        _ => {}
                    }
                }
                break RetrieveProviderResult::Abort(AbortCause::Shutdown);
            }
            match context_ref.url_queue().dequeue().await {
                Ok(Some(entry)) => {
                    retries = retries.saturating_sub(1);
                    if max_age != 0 && entry.age > max_age {
                        log::debug!("Drop {:?} from queue due to age.", entry);
                        continue;
                    }
                    match context_ref.get_link_state(&entry.target).await {
                        Ok(found) => {
                            if let Some(found) = found {
                                if drop_from_queue(context_ref, &entry, &found).await {
                                    missed_domains += 1;
                                    log::debug!("Drop {:?} from queue.", entry);
                                    continue;
                                }
                                if found.typ != Discovered {
                                    missed_domains_cache.push(entry);
                                    missed_domains += 1;
                                    match push_logic_1(
                                        context_ref,
                                        missed_domains,
                                        missed_domains_cache,
                                        &max_miss,
                                        retries
                                    ).await {
                                        RetrieveProviderResult::Ok(cache) => {
                                            missed_domains_cache = cache;
                                            continue;
                                        }
                                        RetrieveProviderResult::Abort(cause) => {
                                            break RetrieveProviderResult::Abort(cause);
                                        }
                                        RetrieveProviderResult::Err(err) => {
                                            break RetrieveProviderResult::Err(err);
                                        }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            break RetrieveProviderResult::Err(QueueExtractionError::LinkState(err));
                        }
                    }
                    match manager.try_reserve_domain(&entry.target).await {
                        Ok(guard) => {
                            if !missed_domains_cache.is_empty() {
                                match context_ref.url_queue().enqueue_all(missed_domains_cache).await {
                                    Err(err) => break RetrieveProviderResult::Err(
                                        QueueExtractionError::QueueError(err)
                                    ),
                                    _ => {}
                                }
                            }
                            break RetrieveProviderResult::Ok(
                                unsafe{GuardedSeedUrlProvider::new_unchecked(guard, entry.target)}
                            );
                        }
                        Err(DomainManagerError::NoDomainError(_)) => {
                            break match context_ref.url_queue().enqueue_all(missed_domains_cache).await {
                                Ok(_) => RetrieveProviderResult::Abort(AbortCause::NoDomain(entry)),
                                Err(err) => RetrieveProviderResult::Err(
                                    QueueExtractionError::QueueError(err)
                                )
                            };
                        }
                        Err(DomainManagerError::AlreadyOccupied(_)) => {
                            missed_domains_cache.push(entry);
                            missed_domains += 1;
                            match push_logic_2(
                                context_ref,
                                missed_domains,
                                missed_domains_cache,
                                &max_miss
                            ).await {
                                RetrieveProviderResult::Ok(cache) => {
                                    missed_domains_cache = cache;
                                    continue;
                                }
                                RetrieveProviderResult::Abort(cause) => {
                                    break RetrieveProviderResult::Abort(cause);
                                }
                                RetrieveProviderResult::Err(err) => {
                                    break RetrieveProviderResult::Err(err);
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    break RetrieveProviderResult::Abort(AbortCause::QueueIsEmpty);
                }
                Err(err) => {
                    break RetrieveProviderResult::Err(QueueExtractionError::QueueError(err));
                }
            }
        };


    }
}


async fn drop_from_queue<C: Context>(context: &C, entry: &UrlQueueElement, state: &LinkState) -> bool {
    match state.typ {
        LinkStateType::Discovered => {false}
        LinkStateType::ProcessedAndStored => {
            let budget = if let Some(ref domain) = entry.target.domain() {
                context.configs().crawl.budget.get_budget_for(domain)
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
async fn push_logic_1<C: Context, T: PartialOrd, const N: usize>(
    context: &C,
    missed_domains: T,
    missed_domains_cache: SmallVec::<[UrlQueueElement; N]>,
    max_miss: &Option<T>,
    retries: usize,
) -> RetrieveProviderResult<SmallVec::<[UrlQueueElement; N]>> {
    if retries == 0 {
        return match context.url_queue().enqueue_all(missed_domains_cache).await {
            Ok(_) => RetrieveProviderResult::Abort(AbortCause::OutOfPullRetries),
            Err(err) => RetrieveProviderResult::Err(QueueExtractionError::QueueError(err))
        };
    }
    push_logic_2(
        context,
        missed_domains,
        missed_domains_cache,
        max_miss
    ).await
}

/// Some private push logic for the macro retrieve_seed, but does not check for retries
async fn push_logic_2<C: Context, T: PartialOrd, const N: usize>(
    context: &C,
    missed_domains: T,
    missed_domains_cache: SmallVec::<[UrlQueueElement; N]>,
    max_miss: &Option<T>,
) -> RetrieveProviderResult<SmallVec::<[UrlQueueElement; N]>> {
    if let Some(unpacked) = max_miss {
        if missed_domains.gt(unpacked) {
            return match context.url_queue().enqueue_all(missed_domains_cache).await {
                Ok(_) => RetrieveProviderResult::Abort(AbortCause::TooManyMisses),
                Err(err) => RetrieveProviderResult::Err(QueueExtractionError::QueueError(err))
            };
        }
    }
    if missed_domains_cache.len() == N {
        return match context.url_queue().enqueue_all(missed_domains_cache).await {
            Err(err) => RetrieveProviderResult::Err(QueueExtractionError::QueueError(err)),
            _ => RetrieveProviderResult::Ok(SmallVec::new())
        }
    }
    RetrieveProviderResult::Ok(missed_domains_cache)
}