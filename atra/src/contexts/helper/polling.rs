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

use std::future::Future;
use crate::contexts::traits::{
    SupportsConfigs, SupportsLinkState, SupportsPolling, SupportsUrlGuarding, SupportsUrlQueue,
};
use crate::link_state::{LinkState, LinkStateKind, LinkStateManager};
use crate::queue::{AbortCause, PollWaiter, PollWaiterFactory, QueueError, QueueExtractionError, UrlQueue, UrlQueueElement, UrlQueuePollResult};
use crate::runtime::ShutdownReceiver;
use crate::url::guard::{GuardianError, UrlGuard, UrlGuardian};
use crate::url::{AtraOriginProvider, UrlWithGuard};
use smallvec::SmallVec;

impl<C> SupportsPolling for C
where
    C: SupportsUrlQueue + SupportsConfigs + SupportsUrlGuarding + SupportsLinkState,
{
    type Guardian = C::Guardian;

    type Error = <<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error;

    async fn poll_next_free_url<'a>(
        &'a self,
        shutdown_handle: impl ShutdownReceiver,
        max_miss: Option<u64>,
    ) -> UrlQueuePollResult<UrlWithGuard<'a, Self::Guardian>, Self::Error> {
        let mut polling = self.url_queue().start_polling();
        if self.url_queue().is_empty().await {
            if !has_possibly_unseen_urls(self, &mut polling).await {
                return UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty)
            }
        }
        const MISSED_KEEPER_CACHE: usize = 32;
        let mut missed_host_cache = Vec::new();
        let mut retries = self.url_queue().len().await;
        let max_age = self.configs().crawl.max_queue_age;
        let mut missed = 0u64;

        let mut cause = loop {
            if shutdown_handle.is_shutdown() {
                break UrlQueuePollResult::Abort(AbortCause::Shutdown);
            }
            if !has_possibly_unseen_urls(self, &mut polling).await {
                break UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty)
            }
            if missed_host_cache.len() > MISSED_KEEPER_CACHE {
                match self.url_queue().enqueue_all(missed_host_cache.drain(..)).await {
                    Ok(_) => {}
                    Err(err) => {
                        break UrlQueuePollResult::Err(err.into())
                    }
                }
            }

            match self.url_queue().dequeue().await {
                Ok(Some(entry)) => {
                    retries = retries.saturating_sub(1);
                    if max_age != 0 && entry.age > max_age {
                        log::debug!("Drop {:?} from queue due to age.", entry);
                        continue;
                    }
                    match self.get_link_state_manager().get_link_state(&entry.target).await {
                        Ok(found) => {
                            if let Some(found) = found {
                                if drop_from_queue(self, &entry, &found).await {
                                    log::debug!("Drop {:?} from queue.", entry);
                                    continue;
                                }
                                if !found.kind.is_discovered() {
                                    missed += 1;
                                    missed_host_cache.push(entry);
                                    if retries == 0 {
                                        break UrlQueuePollResult::Abort(AbortCause::AllDomainsGuarded)
                                    } else if let Some(unpacked) = max_miss {
                                        if missed > unpacked {
                                            break UrlQueuePollResult::Abort(AbortCause::TooManyMisses)
                                        }
                                    }
                                    continue
                                }
                            }
                        }
                        Err(err) => {
                            break UrlQueuePollResult::Err(QueueExtractionError::LinkState(
                                err,
                            ));
                        }
                    }
                    match self.get_guardian().try_reserve(&entry.target).await {
                        Ok(guard) => {
                            break UrlQueuePollResult::Ok(unsafe {
                                UrlWithGuard::new_unchecked(guard, entry.target, entry.is_seed)
                            });
                        }
                        Err(GuardianError::NoOriginError(_)) => {
                            break UrlQueuePollResult::Abort(AbortCause::NoHost(entry))
                        }
                        Err(GuardianError::AlreadyOccupied(_)) => {
                            missed_host_cache.push(entry.clone());
                            missed += 1;
                            if let Some(unpacked) = max_miss {
                                if missed > unpacked {
                                    break UrlQueuePollResult::Abort(AbortCause::TooManyMisses)
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    if !has_possibly_unseen_urls(self, &mut polling).await {
                        if missed != 0 {
                            break UrlQueuePollResult::Abort(AbortCause::AllDomainsGuarded);
                        } else {
                            break UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty);
                        }
                    }
                }
                Err(err) => {
                    break UrlQueuePollResult::Err(QueueExtractionError::QueueError(err));
                }
            }
        };

        if !missed_host_cache.is_empty() {
            if !cause.is_ok() {
                let mut to_add = Vec::with_capacity(missed_host_cache.len());
                let mut iter = missed_host_cache.into_iter();
                while let Some(next) = iter.next() {
                    match self.get_guardian().try_reserve(&next.target).await {
                        Ok(guard) => {
                            cause = UrlQueuePollResult::Ok(unsafe {
                                UrlWithGuard::new_unchecked(guard, next.target, next.is_seed)
                            });
                        }
                        Err(_) => {
                            to_add.push(next);
                        }
                    }
                    if cause.is_ok() {
                        break
                    }
                }
                to_add.extend(iter);
                missed_host_cache = to_add;
            }
            match self.url_queue().enqueue_all(missed_host_cache).await {
                Ok(_) => cause,
                Err(err) => {
                    if cause.is_ok() {
                        let unwrapped = cause.unwrap();
                        let (url, is_seed) = unwrapped.into_seed();
                        /// We are already in an error state.
                        /// Hence, we can just ignore whatever happens.
                        let _ = self.url_queue().enqueue(
                            UrlQueueElement::new(
                                is_seed,
                                0,
                                false,
                                url
                            )
                        );
                    }
                    UrlQueuePollResult::Err(QueueExtractionError::QueueError(err))
                },
            }
        } else {
            cause
        }
    }
}

/// Returns true if there are possibly more unseen urls.
async fn has_possibly_unseen_urls<C: SupportsUrlQueue>(context: &C, polling: &mut PollWaiter<'_>) -> bool {
    if polling.has_other_waiters() {
        if !context.url_queue().is_empty().await {
            return true
        }
        loop {
            let other_waiter_count = polling.wait_for_other_waiter_count_changed().await;
            let queue_is_empty = context.url_queue().is_empty().await;
            if other_waiter_count == 0 {
                break queue_is_empty;
            } else if !queue_is_empty {
                break true
            }
        }
    } else {
        !context.url_queue().is_empty().await
    }
}

async fn drop_from_queue<C: SupportsConfigs>(
    context: &C,
    entry: &UrlQueueElement,
    state: &LinkState,
) -> bool {
    match state.kind {
        LinkStateKind::Discovered => false,
        LinkStateKind::ProcessedAndStored => {
            let budget = if let Some(origin) = entry.target.atra_origin() {
                context.configs().crawl.budget.get_budget_for(&origin)
            } else {
                &context.configs().crawl.budget.default
            };
            budget.get_recrawl_interval().is_none()
        }
        LinkStateKind::InternalError
        | LinkStateKind::Unset
        | LinkStateKind::Crawled
        | LinkStateKind::ReservedForCrawl => true,
        LinkStateKind::Unknown(id) => {
            log::debug!("Some unknown link state of type {id} was found!");
            true
        }
    }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;
    use std::time::Duration;
    use crate::config::{Configs, CrawlConfig, PathsConfig, SessionConfig, SystemConfig};
    use crate::config::crawl::CrawlBudget;
    use crate::contexts::traits::{SupportsConfigs, SupportsLinkState, SupportsPolling, SupportsUrlGuarding, SupportsUrlQueue};
    use crate::contexts::BaseContext;
    use crate::queue::{AbortCause, QueueExtractionError, UrlQueue, UrlQueueElement, UrlQueuePollResult};
    use crate::test_impls::{InMemoryLinkStateManager, TestUrlQueue};
    use crate::url::guard::{GuardianError, InMemoryUrlGuardian, UrlGuardian};
    use crate::url::{UrlWithDepth};

    struct Fake {
        queue: TestUrlQueue,
        configs: Configs,
        guard: InMemoryUrlGuardian,
        link_state_manager: InMemoryLinkStateManager
    }

    impl Fake {
        pub fn new(configs: Configs) -> Self {
            Self {
                queue: TestUrlQueue::default(),
                configs,
                guard: InMemoryUrlGuardian::new(),
                link_state_manager: InMemoryLinkStateManager::new()
            }
        }
    }

    impl BaseContext for Fake {}

    impl SupportsUrlQueue for Fake {
        type UrlQueue = TestUrlQueue;

        async fn can_poll(&self) -> bool {
            !self.queue.is_empty().await
        }

        fn url_queue(&self) -> &Self::UrlQueue {
            &self.queue
        }
    }

    impl SupportsConfigs for Fake {
        fn configs(&self) -> &Configs {
            &self.configs
        }
    }

    impl SupportsUrlGuarding for Fake {
        type Guardian = InMemoryUrlGuardian;

        fn get_guardian(&self) -> &Self::Guardian {
            &self.guard
        }
    }

    impl SupportsLinkState for Fake {
        type LinkStateManager = InMemoryLinkStateManager;
        fn get_link_state_manager(&self) -> &Self::LinkStateManager {
            &self.link_state_manager
        }
    }

    fn create_configs(
        max_queue_age: Option<u32>,
        budget_setting: Option<CrawlBudget>
    ) -> Configs {
        let mut cfg = CrawlConfig::default();
        if let Some(max_queue_age) = max_queue_age {
            cfg.max_queue_age = max_queue_age;
        }

        if let Some(budget) = budget_setting {
            cfg.budget = budget;
        }

        Configs::new(
            SystemConfig::default(),
            PathsConfig::default(),
            SessionConfig::default(),
            cfg
        )
    }

    #[tokio::test]
    async fn polling_works() {
        let cfg = create_configs(None, None);
        let fake = Arc::new(Fake::new(cfg));
        fake.queue.enqueue_all(
            [
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_seed("https://www.test1.de").unwrap(),
                ),
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_seed("https://www.test2.de").unwrap(),
                ),
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_seed("https://www.test3.de").unwrap(),
                ),
                UrlQueueElement::new(
                    false,
                    0,
                    false,
                    UrlWithDepth::from_seed("https://www.test2.de/uniform").unwrap(),
                ),
                UrlQueueElement::new(
                    false,
                    0,
                    false,
                    UrlWithDepth::from_seed("https://www.test3.de/katze").unwrap(),
                ),
                // UrlQueueElement::new(
                //     true,
                //     0,
                //     false,
                //     UrlWithDepth::from_seed("https://www.test4.de/").unwrap(),
                // ),
            ]
        ).await.unwrap();


        let next1 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        let next2 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        let next3 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();

        assert_eq!("https://www.test1.de/", next1.seed_url().as_str());
        assert_eq!("https://www.test2.de/", next2.seed_url().as_str());
        assert_eq!("https://www.test3.de/", next3.seed_url().as_str());
        assert_eq!(2, fake.queue.len().await);

        let fake2 = fake.clone();
        let result = tokio::spawn(async move {
            assert_eq!(2, fake2.queue.len().await);
            match fake2.poll_next_free_url_no_shutdown(None).await {
                UrlQueuePollResult::Ok(ok) => {
                    panic!("Ok for {}", ok.seed_url())
                }
                UrlQueuePollResult::Abort(AbortCause::AllDomainsGuarded) => {
                    println!("All domains guarded");
                }
                UrlQueuePollResult::Abort(ab) => {
                    panic!("Abort for {}", ab)
                }
                UrlQueuePollResult::Err(err) => {
                    match err {
                        QueueExtractionError::GuardianError(err) => {
                            match err {
                                GuardianError::NoOriginError(err) => {
                                    panic!("No origin found! {err}")
                                }
                                GuardianError::AlreadyOccupied(err) => {
                                    panic!("This error should never occur! {err}")
                                }
                            }
                        }
                        QueueExtractionError::LinkState(err) => {
                            panic!("{err}")
                        }
                        QueueExtractionError::QueueError(err) => {
                            panic!("{err}")
                        }
                    }
                }
            }
            let mut inp = fake2.get_guardian().subscribe();
            inp.recv().await.unwrap();
            match fake2.poll_next_free_url_no_shutdown(None).await {
                UrlQueuePollResult::Ok(ok) => {
                    panic!("Ok for {}", ok.seed_url())
                }
                UrlQueuePollResult::Abort(ab) => {
                    panic!("Abort for {}", ab)
                }
                UrlQueuePollResult::Err(err) => {
                    match err {
                        QueueExtractionError::GuardianError(err) => {
                            match err {
                                GuardianError::NoOriginError(err) => {
                                    panic!("No origin found! {err}")
                                }
                                GuardianError::AlreadyOccupied(err) => {
                                    println!("Occupied: {err}");
                                }
                            }
                        }
                        QueueExtractionError::LinkState(err) => {
                            panic!("{err}")
                        }
                        QueueExtractionError::QueueError(err) => {
                            panic!("{err}")
                        }
                    }
                }
            }

            inp.recv().await.unwrap();

            match fake2.poll_next_free_url_no_shutdown(None).await {
                UrlQueuePollResult::Ok(ok) => {
                    assert_eq!("https://www.test3.de/katze/", ok.seed_url().as_str());
                    println!("Process: {}", ok.seed_url().as_str());
                }
                UrlQueuePollResult::Abort(ab) => {
                    panic!("Abort for {}", ab)
                }
                UrlQueuePollResult::Err(err) => {
                    match err {
                        QueueExtractionError::GuardianError(err) => {
                            match err {
                                GuardianError::NoOriginError(err) => {
                                    panic!("No origin found! {err}")
                                }
                                GuardianError::AlreadyOccupied(err) => {
                                    panic!("Already occupied! {err}");
                                }
                            }
                        }
                        QueueExtractionError::LinkState(err) => {
                            panic!("{err}")
                        }
                        QueueExtractionError::QueueError(err) => {
                            panic!("{err}")
                        }
                    }
                }
            }
        });

        println!("Drop {}", next1.seed_url().as_str());
        drop(next1);
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("Drop {}", next3.seed_url().as_str());
        drop(next3);
        let mut subs = fake.get_guardian().subscribe();
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("Drop {}", next2.seed_url().as_str());
        drop(next2);
        subs.recv().await.unwrap();
        let url = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        println!("{}", url.seed_url().as_str());
        result.await.unwrap();
    }
}
