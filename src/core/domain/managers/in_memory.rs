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
use case_insensitive_string::CaseInsensitiveString;
use compact_str::ToCompactString;
use crate::core::domain::entry::DomainEntry;
use crate::core::domain::errors::DomainManagerError;
use crate::core::domain::guard::DomainGuard;
use crate::core::domain::{DomainManager, GuardPoisonedError};
use crate::core::domain::manager::InternalDomainManager;
use crate::core::UrlWithDepth;

/// Manages the crawl state of the domains in the current crawl
#[derive(Debug, Default)]
pub struct InMemoryDomainManager {
    data_holder: Arc<tokio::sync::RwLock<HashMap<CaseInsensitiveString, DomainEntry>>>,
}

impl InMemoryDomainManager {
    pub fn new() -> Self {
        Self { data_holder: Default::default() }
    }
}

impl InternalDomainManager for InMemoryDomainManager {
    fn release_domain(&self, domain: CaseInsensitiveString) {
        // This makes sure, that we never fail with our spawned task.
        let shared = self.data_holder.clone();
        tokio::spawn(
            async move {
                let mut holder = shared.write().await;
                if let Some(value) = holder.get_mut(&domain) {
                    value.is_in_use = false;
                    value.last_modification = Some(SystemTime::now());
                } else {
                    unreachable!();
                }
            }
        );
    }
}

impl DomainManager for InMemoryDomainManager {

    async fn try_reserve_domain<'a>(&'a self, url: &UrlWithDepth) -> Result<DomainGuard<'a, Self>, DomainManagerError> {
        let domain = url.domain().ok_or_else(|| DomainManagerError::NoDomainError(url.clone()))?;
        let mut holder = self.data_holder.write().await;
        if let Some(found) = holder.get_mut(&domain) {
            if found.is_in_use {
                return Err(DomainManagerError::AlreadyOccupied(domain))
            }
            let reserved_at = SystemTime::now();
            found.last_modification = Some(reserved_at.clone());
            found.depth = found.depth.merge_to_lowes(url.depth());
            found.is_in_use = true;

            return Ok(
                DomainGuard {
                    reserved_at,
                    domain_manager: self as *const InMemoryDomainManager,
                    domain_entry: found.clone(),
                    domain,
                    _marker: PhantomData
                }
            )
        }
        let reserved_at = SystemTime::now();
        let domain_entry = DomainEntry {
            is_in_use: true,
            last_modification: None,
            depth: url.depth().clone()
        };
        holder.insert(
            domain.clone(),
            domain_entry.clone()
        );
        Ok(
            DomainGuard {
                reserved_at,
                domain_manager: self as *const InMemoryDomainManager,
                domain,
                domain_entry,
                _marker: PhantomData
            }
        )
    }

    async fn can_provide_additional_value(&self, url: &UrlWithDepth) -> bool {
        match url.domain() {
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

    async fn knows_domain(&self, url: &UrlWithDepth) -> Option<bool> {
        let domain = url.domain()?;
        let holder = self.data_holder.read().await;
        Some(holder.contains_key(&domain))
    }

    async fn current_domain_state(&self, url: &UrlWithDepth) -> Option<DomainEntry> {
        let domain = url.domain()?;
        let holder = self.data_holder.read().await;
        holder.get(&domain).cloned()
    }

    async fn currently_reserved_domains(&self) -> Vec<CaseInsensitiveString> {
        let read = self.data_holder.read().await;
        read.iter().filter_map(|(domain, state)| {
            if state.is_in_use {
                Some(domain.clone())
            } else {
                None
            }
        }).collect()
    }


    async fn check_if_poisoned<'a>(&self, guard: &DomainGuard<'a, Self>) -> Result<(), GuardPoisonedError> {
        let read = self.data_holder.read().await;
        if let Some(found) = read.get(&guard.domain) {
            if found.is_in_use {
                if let Some(ref modification) = found.last_modification {
                    if guard.reserved_at.eq(modification) {
                        Ok(())
                    } else {
                        Err(GuardPoisonedError::WrongTimestampSet(guard.domain.as_ref().to_compact_string(), guard.reserved_at, modification.clone()))
                    }
                } else {
                    Err(GuardPoisonedError::NoTimestampSet(guard.domain.as_ref().to_compact_string()))
                }
            } else {
                Err(GuardPoisonedError::InUseNotSet(guard.domain.as_ref().to_compact_string()))
            }
        } else {
            Err(GuardPoisonedError::DomainMissing(guard.domain.as_ref().to_compact_string()))
        }
    }
}

impl Clone for InMemoryDomainManager {
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
    use crate::core::domain::{DomainManager, DomainManagerError};
    use crate::core::UrlWithDepth;

    #[tokio::test]
    async fn the_domain_works_as_expected() {
        let domain_manager = super::InMemoryDomainManager::new();
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
            let domain_manager = domain_manager.clone();

            match a {
                Position::Last => {
                    let domains = domains.clone();
                    handles.push(
                        tokio::task::spawn(
                            async move {
                                for current in &domains {
                                    println!("{:?}", domain_manager.current_domain_state(current).await)
                                }
                                println!("Waiting!");
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                println!("Finished!");
                                let wait_result2 = c2.wait().await;
                                for current in &domains {
                                    println!("{} - {:?}", current, domain_manager.current_domain_state(current).await)
                                }
                                let succ = domain_manager.try_reserve_domain(&url).await;
                                let found_text = match &succ {
                                    Ok(_) => {
                                        "successfull"
                                    }
                                    Err(DomainManagerError::NoDomainError(url)) => {
                                        panic!("The no domain error for {url} should not occur!")
                                    }
                                    Err(DomainManagerError::AlreadyOccupied(_)) => {
                                        "unsucessfull"
                                    }
                                };
                                for current in &domains {
                                    println!("{} - {:?}", current, domain_manager.current_domain_state(current).await)
                                }
                                println!("Was reserve domain for {} was {}", url, found_text);

                                (None, wait_result2, succ.is_err(), url.domain().unwrap(), url)
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
                                let barrier = domain_manager.try_reserve_domain(&url).await.unwrap();
                                let wait_result2 = c2.wait().await;
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                (Some(wait_result1), wait_result2, true, barrier.domain().clone(), url)
                            }
                        )
                    )
                }
            }
        }

        for handle in handles {
            let (_, _, was_correctly_borrowed, domain, url) = handle.await.unwrap();
            assert!(was_correctly_borrowed, "Expected that {} for {} was correctly bowwored", domain.inner(), url)
        }

        for current in &domains {
            println!("{} - {:?}", current, domain_manager.current_domain_state(current).await)
        }
    }
}