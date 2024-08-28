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


use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::SystemTime;
use crate::core::origin::entry::OriginEntry;
use crate::core::origin::errors::OriginManagerError;
use crate::core::origin::guard::OriginGuard;
use crate::core::origin::{OriginManager, GuardPoisonedError, AtraOriginProvider};
use crate::core::origin::manager::InternalOriginManager;
use crate::core::origin::AtraUrlOrigin;
use crate::core::UrlWithDepth;

/// Manages the crawl state of the domains in the current crawl
#[derive(Debug, Default)]
pub struct InMemoryOriginManager {
    data_holder: Arc<tokio::sync::RwLock<HashMap<AtraUrlOrigin, OriginEntry>>>,
}

impl InMemoryOriginManager {
    pub fn new() -> Self {
        Self { data_holder: Default::default() }
    }
}

impl InternalOriginManager for InMemoryOriginManager {
    fn release(&self, origin: AtraUrlOrigin) {
        // This makes sure, that we never fail with our spawned task.
        let shared = self.data_holder.clone();
        tokio::spawn(
            async move {
                let mut holder = shared.write().await;
                if let Some(value) = holder.get_mut(&origin) {
                    value.is_in_use = false;
                    value.last_modification = Some(SystemTime::now());
                } else {
                    unreachable!();
                }
            }
        );
    }
}

impl OriginManager for InMemoryOriginManager {

    async fn try_reserve<'a>(&'a self, url: &UrlWithDepth) -> Result<OriginGuard<'a, Self>, OriginManagerError> {
        let origin = url.atra_origin().ok_or_else(|| OriginManagerError::NoOriginError(url.clone()))?;
        let mut holder = self.data_holder.write().await;
        if let Some(found) = holder.get_mut(&origin) {
            if found.is_in_use {
                return Err(OriginManagerError::AlreadyOccupied(origin))
            }
            let reserved_at = SystemTime::now();
            found.last_modification = Some(reserved_at.clone());
            found.depth = found.depth.merge_to_lowes(url.depth());
            found.is_in_use = true;

            return Ok(
                OriginGuard {
                    reserved_at,
                    origin_manager: self as *const InMemoryOriginManager,
                    entry: found.clone(),
                    origin,
                    _marker: PhantomData
                }
            )
        }
        let reserved_at = SystemTime::now();
        let entry = OriginEntry {
            is_in_use: true,
            last_modification: None,
            depth: url.depth().clone()
        };
        holder.insert(
            origin.clone(),
            entry.clone()
        );
        Ok(
            OriginGuard {
                reserved_at,
                origin_manager: self as *const InMemoryOriginManager,
                origin,
                entry,
                _marker: PhantomData
            }
        )
    }

    async fn can_provide_additional_value(&self, url: &UrlWithDepth) -> bool {
        match url.atra_origin() {
            None => {false}
            Some(ref value) => {
                let holder = self.data_holder.read().await;
                match holder.get(value) {
                    None => {true}
                    Some(value) => {
                        url.depth() < &value.depth
                    }
                }
            }
        }
    }

    async fn knows_origin(&self, url: &UrlWithDepth) -> Option<bool> {
        let host = url.atra_origin()?;
        let holder = self.data_holder.read().await;
        Some(holder.contains_key(&host))
    }

    async fn current_origin_state(&self, url: &UrlWithDepth) -> Option<OriginEntry> {
        let host = url.atra_origin()?;
        let holder = self.data_holder.read().await;
        holder.get(&host).cloned()
    }

    async fn currently_reserved_origins(&self) -> Vec<AtraUrlOrigin> {
        let read = self.data_holder.read().await;
        read.iter().filter_map(|(host, state)| {
            if state.is_in_use {
                Some(host.clone())
            } else {
                None
            }
        }).collect()
    }


    async fn check_if_poisoned<'a>(&self, guard: &OriginGuard<'a, Self>) -> Result<(), GuardPoisonedError> {
        let read = self.data_holder.read().await;
        if let Some(found) = read.get(&guard.origin) {
            if found.is_in_use {
                if let Some(ref modification) = found.last_modification {
                    if guard.reserved_at.eq(modification) {
                        Ok(())
                    } else {
                        Err(GuardPoisonedError::WrongTimestampSet(guard.origin.clone(), guard.reserved_at, modification.clone()))
                    }
                } else {
                    Err(GuardPoisonedError::NoTimestampSet(guard.origin.clone()))
                }
            } else {
                Err(GuardPoisonedError::InUseNotSet(guard.origin.clone()))
            }
        } else {
            Err(GuardPoisonedError::OriginMissing(guard.origin.clone()))
        }
    }
}

impl Clone for InMemoryOriginManager {
    fn clone(&self) -> Self {
        Self{data_holder: self.data_holder.clone()}
    }
}


#[cfg(test)]
mod test {
    use std::sync::Arc;
    use std::time::Duration;
    use itertools::{Itertools, Position};
    use smallvec::SmallVec;
    use crate::core::origin::{AtraOriginProvider, OriginManager, OriginManagerError};
    use crate::core::UrlWithDepth;

    #[tokio::test]
    async fn the_domain_works_as_expected() {
        let host_manager = super::InMemoryOriginManager::new();
        let domains = [
            "https://www.google.de".parse::<UrlWithDepth>().unwrap(),
            "https://www.ebay.de".parse::<UrlWithDepth>().unwrap(),
            "https://www.youtube.com/".parse::<UrlWithDepth>().unwrap(),
            "https://www.germany.de/".parse::<UrlWithDepth>().unwrap(),
            "https://www.gradle.org/test/".parse::<UrlWithDepth>().unwrap(),
            "https://www.hello.info/".parse::<UrlWithDepth>().unwrap(),
            "https://www.amazon.co.uk/prod?v=1".parse::<UrlWithDepth>().unwrap(),
            "https://www.ebay.de/cat".parse::<UrlWithDepth>().unwrap(),
        ];

        let barrier1 = Arc::new(tokio::sync::Barrier::new(domains.len() - 1));
        let barrier2 = Arc::new(tokio::sync::Barrier::new(domains.len()));

        let mut handles = SmallVec::<[_; 16]>::new();
        for (a, b) in domains.iter().with_position() {
            let url = b.clone();
            let c2 = barrier2.clone();
            let host_manager = host_manager.clone();

            match a {
                Position::Last => {
                    let hosts = domains.clone();
                    handles.push(
                        tokio::task::spawn(
                            async move {
                                for current in &hosts {
                                    println!("{:?}", host_manager.current_origin_state(current).await)
                                }
                                println!("Waiting!");
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                println!("Finished!");
                                let wait_result2 = c2.wait().await;
                                for current in &hosts {
                                    println!("{} - {:?}", current, host_manager.current_origin_state(current).await)
                                }
                                let succ = host_manager.try_reserve(&url).await;
                                let found_text = match &succ {
                                    Ok(_) => {
                                        "successfull"
                                    }
                                    Err(OriginManagerError::NoOriginError(url)) => {
                                        panic!("The no domain error for {url} should not occur!")
                                    }
                                    Err(OriginManagerError::AlreadyOccupied(_)) => {
                                        "unsucessfull"
                                    }
                                };
                                for current in &hosts {
                                    println!("{} - {:?}", current, host_manager.current_origin_state(current).await)
                                }
                                println!("Was reserved origin for {} was {}", url, found_text);

                                (None, wait_result2, succ.is_err(), url.atra_origin().unwrap(), url)
                            }
                        )
                    )
                }
                Position::Only => unreachable!(),
                _ => {
                    let c1 = barrier1.clone();
                    handles.push(
                        tokio::task::spawn(
                            async move {
                                let wait_result1 = c1.wait().await;
                                let barrier = host_manager.try_reserve(&url).await.unwrap();
                                let wait_result2 = c2.wait().await;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                (Some(wait_result1), wait_result2, true, barrier.origin().to_owned(), url)
                            }
                        )
                    )
                }
            }
        }

        for handle in handles {
            let (_, _, was_correctly_borrowed, origin, url) = handle.await.unwrap();
            assert!(was_correctly_borrowed, "Expected that {} for {} was correctly borrowed", origin, url)
        }

        for current in &domains {
            println!("{} - {:?}", current, host_manager.current_origin_state(current).await)
        }
    }
}