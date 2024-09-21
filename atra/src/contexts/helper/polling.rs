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

use crate::contexts::traits::{
    SupportsConfigs, SupportsLinkState, SupportsPolling, SupportsUrlGuarding, SupportsUrlQueue,
};
use crate::link_state::{LinkStateKind, LinkStateLike, LinkStateManager};
use crate::queue::{
    AbortCause, EnqueueCalled, QueueExtractionError, UrlQueue, UrlQueueElement, UrlQueueElementRef,
    UrlQueuePollResult,
};
use crate::runtime::ShutdownReceiver;
use crate::sync::barrier::ContinueOrStop;
use crate::url::guard::{GuardianError, UrlGuardian};
use crate::url::{AtraOriginProvider, UrlWithDepth, UrlWithGuard};
use std::error::Error;
use std::time::Duration;
use tokio::select;
use tokio::sync::watch::Receiver;
use tokio::time::Instant;

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
        let queue = self.url_queue();
        let guardian = self.get_guardian();
        let manager = self.get_link_state_manager();
        const MISSED_KEEPER_CACHE: usize = 32;
        let mut missed_host_cache: Vec<UrlQueueElementRef<UrlWithDepth>> =
            Vec::with_capacity(MISSED_KEEPER_CACHE);
        let max_age = self.configs().crawl.max_queue_age;
        let mut waiter: Option<Receiver<EnqueueCalled>> = None;
        let mut missed = 0;
        let max_miss = max_miss.unwrap_or(u64::MAX);
        let mut force_clean_cache = false;

        async fn process_entries<'a, G, E>(
            guardian: &'a G,
            missed_host_cache: &mut Vec<UrlQueueElementRef<'a, UrlWithDepth>>,
        ) -> Option<UrlQueuePollResult<UrlWithGuard<'a, G>, E>>
        where
            G: UrlGuardian,
            E: Error,
        {
            for entry in missed_host_cache.drain(..) {
                match guardian.try_reserve(&entry.target).await {
                    Ok(guard) => {
                        let result = unsafe {
                            let entry = entry.take();
                            UrlWithGuard::new_unchecked(guard, entry.target, entry.is_seed)
                        };
                        return Some(UrlQueuePollResult::Ok(result));
                    }
                    Err(GuardianError::NoOriginError(_)) => {
                        return Some(UrlQueuePollResult::Abort(AbortCause::NoHost(entry.take())))
                    }
                    Err(GuardianError::AlreadyOccupied(_)) => drop(entry),
                }
            }
            None
        }

        let result = loop {
            if queue.is_empty().await && !queue.has_floating_urls() {
                break UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty);
            }
            if missed > max_miss {
                break UrlQueuePollResult::Abort(AbortCause::TooManyMisses);
            }
            if missed_host_cache.len() == missed_host_cache.capacity() || force_clean_cache {
                if let Some(result) = process_entries(guardian, &mut missed_host_cache).await {
                    break result;
                }
                force_clean_cache = false;
            }

            match queue.dequeue().await {
                Ok(Some(entry)) => {
                    // let it age
                    if max_age != 0 && entry.age > max_age {
                        log::debug!("Drop {:?} from queue due to age.", entry.target);
                        entry.drop_from_queue();
                        continue;
                    }

                    match manager.get_link_state(&entry.target).await {
                        Ok(Some(found)) => {
                            if drop_from_queue(self, &entry, &found).await {
                                log::debug!("Drop {:?} from queue.", entry.target);
                                entry.drop_from_queue();
                                continue;
                            }
                        }
                        Err(err) => {
                            break UrlQueuePollResult::Err(QueueExtractionError::LinkState(err));
                        }
                        _ => {}
                    }

                    match guardian.try_reserve(&entry.target).await {
                        Ok(guard) => {
                            let result = unsafe {
                                let entry = entry.take();
                                UrlWithGuard::new_unchecked(guard, entry.target, entry.is_seed)
                            };
                            break UrlQueuePollResult::Ok(result);
                        }
                        Err(GuardianError::NoOriginError(_)) => {
                            break UrlQueuePollResult::Abort(AbortCause::NoHost(entry.take()))
                        }
                        Err(GuardianError::AlreadyOccupied(_)) => {
                            missed += 1;
                            missed_host_cache.push(entry);
                        }
                    }
                }
                Ok(None) => {
                    if queue.has_floating_urls() {
                        let guard_changes =
                            waiter.get_or_insert_with(|| queue.subscribe_to_change());

                        let result = select! {
                            _ = guard_changes.changed() => {
                                ContinueOrStop::Continue(false)
                            }
                            _ = shutdown_handle.wait() => {
                                 ContinueOrStop::Cancelled(
                                    UrlQueuePollResult::Abort(AbortCause::Shutdown)
                                )
                            }
                            _ = tokio::time::sleep_until(Instant::now() + Duration::from_millis(1_000)) => {
                                ContinueOrStop::Continue(true)
                            }
                        };
                        match result {
                            ContinueOrStop::Cancelled(err) => break err,
                            ContinueOrStop::Continue(fd) => force_clean_cache = fd,
                        }
                    }
                    continue;
                }
                Err(err) => {
                    break UrlQueuePollResult::Err(QueueExtractionError::QueueError(err));
                }
            }
        };

        if !result.is_ok() && !missed_host_cache.is_empty() {
            if let Some(result2) = process_entries(guardian, &mut missed_host_cache).await {
                if result2.is_ok() {
                    return result2;
                }
            }
        }
        return result;
    }
}

async fn drop_from_queue<C: SupportsConfigs>(
    context: &C,
    entry: &UrlQueueElement,
    state: &impl LinkStateLike,
) -> bool {
    match state.kind() {
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
    use crate::config::crawl::CrawlBudget;
    use crate::config::{Config, CrawlConfig, PathsConfig, SessionConfig, SystemConfig};
    use crate::contexts::traits::{
        SupportsConfigs, SupportsLinkState, SupportsPolling, SupportsUrlGuarding, SupportsUrlQueue,
    };
    use crate::contexts::BaseContext;
    use crate::queue::{QueueExtractionError, UrlQueue, UrlQueueElement, UrlQueuePollResult};
    use crate::test_impls::{InMemoryLinkStateManager, TestUrlQueue};
    use crate::url::guard::{InMemoryUrlGuardian, UrlGuardian};
    use crate::url::UrlWithDepth;
    use std::sync::Arc;
    use std::time::Duration;

    struct Fake {
        queue: TestUrlQueue,
        configs: Config,
        guard: InMemoryUrlGuardian,
        link_state_manager: InMemoryLinkStateManager,
    }

    impl Fake {
        pub fn new(configs: Config) -> Self {
            Self {
                queue: TestUrlQueue::default(),
                configs,
                guard: InMemoryUrlGuardian::new(),
                link_state_manager: InMemoryLinkStateManager::new(),
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
        fn configs(&self) -> &Config {
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

    fn create_configs(max_queue_age: Option<u32>, budget_setting: Option<CrawlBudget>) -> Config {
        let mut cfg = CrawlConfig::default();
        if let Some(max_queue_age) = max_queue_age {
            cfg.max_queue_age = max_queue_age;
        }

        if let Some(budget) = budget_setting {
            cfg.budget = budget;
        }

        Config::new(
            SystemConfig::default(),
            PathsConfig::default(),
            SessionConfig::default(),
            cfg,
        )
    }

    #[tokio::test]
    async fn polling_works() {
        let cfg = create_configs(None, None);
        let fake = Arc::new(Fake::new(cfg));
        fake.queue
            .enqueue_all([
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_url("https://www.test1.de").unwrap(),
                ),
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_url("https://www.test2.de").unwrap(),
                ),
                UrlQueueElement::new(
                    true,
                    0,
                    false,
                    UrlWithDepth::from_url("https://www.test3.de").unwrap(),
                ),
                UrlQueueElement::new(
                    false,
                    0,
                    false,
                    UrlWithDepth::from_url("https://www.test2.de/uniform").unwrap(),
                ),
                UrlQueueElement::new(
                    false,
                    0,
                    false,
                    UrlWithDepth::from_url("https://www.test3.de/katze").unwrap(),
                ),
                // UrlQueueElement::new(
                //     true,
                //     0,
                //     false,
                //     UrlWithDepth::from_seed("https://www.test4.de/").unwrap(),
                // ),
            ])
            .await
            .unwrap();

        let next1 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        let next2 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        let next3 = fake.poll_next_free_url_no_shutdown(None).await.unwrap();

        assert_eq!("https://www.test1.de/", next1.seed_url().try_as_str());
        assert_eq!("https://www.test2.de/", next2.seed_url().try_as_str());
        assert_eq!("https://www.test3.de/", next3.seed_url().try_as_str());
        assert_eq!(2, fake.queue.len().await);

        let fake2 = fake.clone();
        let mut inp = fake2.get_guardian().subscribe();
        let result = tokio::spawn(async move {
            assert_eq!(2, fake2.queue.len().await);
            match fake2.poll_next_free_url_no_shutdown(None).await {
                UrlQueuePollResult::Ok(ok) => {
                    assert_eq!("https://www.test3.de/katze", ok.seed_url().try_as_str());
                    println!("Process: {}", ok.seed_url().try_as_str());
                }
                UrlQueuePollResult::Abort(ab) => {
                    panic!("Abort for {}", ab)
                }
                UrlQueuePollResult::Err(err) => match err {
                    QueueExtractionError::LinkState(err) => {
                        panic!("{err}")
                    }
                    QueueExtractionError::QueueError(err) => {
                        panic!("{err}")
                    }
                },
            }
            inp.changed().await.unwrap();
            match fake2.poll_next_free_url_no_shutdown(None).await {
                UrlQueuePollResult::Ok(ok) => {
                    assert_eq!("https://www.test2.de/uniform", ok.seed_url().try_as_str());
                    println!("Process {}", ok.seed_url().try_as_str())
                }
                UrlQueuePollResult::Abort(ab) => {
                    panic!("Abort for {}", ab)
                }
                UrlQueuePollResult::Err(err) => match err {
                    QueueExtractionError::LinkState(err) => {
                        panic!("{err}")
                    }
                    QueueExtractionError::QueueError(err) => {
                        panic!("{err}")
                    }
                },
            }
        });

        println!("Drop {}", next1.seed_url().try_as_str());
        drop(next1);
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("Drop {}", next3.seed_url().try_as_str());
        drop(next3);
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("Drop {}", next2.seed_url().try_as_str());
        drop(next2);
        tokio::time::sleep(Duration::from_secs(2)).await;
        // let url = fake.poll_next_free_url_no_shutdown(None).await.unwrap();
        // println!("{}", url.seed_url().as_str());
        let result = result.await;
        println!("{:?}", result)
    }
}
