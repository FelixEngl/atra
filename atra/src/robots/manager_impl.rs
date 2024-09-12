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

use crate::client::Client;
use crate::database::DBActionType::{Delete, Read, Write};
use crate::database::RawDatabaseError;
use crate::robots::{CachedRobots, RobotsError, RobotsManager};
use crate::url::UrlWithDepth;
use crate::url::{AtraOriginProvider, AtraUrlOrigin};
use crate::{db_health_check, declare_column_families};
use rocksdb::{BoundColumnFamily, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use texting_robots::{get_robots_url, Robot};
use time::{Duration, OffsetDateTime};
use tokio::task::yield_now;

/// Allows to share the threadsafe variants of  [RobotsManager] over threads
#[derive(Debug, Clone)]
pub enum ShareableRobotsManager {
    InMemory(Arc<InMemoryRobotsManager>),
    OffMemory(Arc<OffMemoryRobotsManager>),
}

impl RobotsManager for ShareableRobotsManager {
    async fn get(
        &self,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError> {
        match self {
            ShareableRobotsManager::InMemory(value) => value.get(agent, url, max_age).await,
            ShareableRobotsManager::OffMemory(value) => value.get(agent, url, max_age).await,
        }
    }

    async fn get_or_retrieve(
        &self,
        client: &Client,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError> {
        match self {
            ShareableRobotsManager::InMemory(value) => {
                value.get_or_retrieve(client, agent, url, max_age).await
            }
            ShareableRobotsManager::OffMemory(value) => {
                value.get_or_retrieve(client, agent, url, max_age).await
            }
        }
    }
}

impl From<OffMemoryRobotsManager> for ShareableRobotsManager {
    fn from(value: OffMemoryRobotsManager) -> Self {
        Self::OffMemory(Arc::new(value))
    }
}

impl From<InMemoryRobotsManager> for ShareableRobotsManager {
    fn from(value: InMemoryRobotsManager) -> Self {
        Self::InMemory(Arc::new(value))
    }
}

/// An in memory variant of a robots.txt manager
/// Ideal for smaller crawls
#[derive(Debug, Default)]
pub struct InMemoryRobotsManager {
    cache: tokio::sync::RwLock<HashMap<AtraUrlOrigin, Arc<CachedRobots>>>,
}

impl InMemoryRobotsManager {
    pub fn new() -> Self {
        Self {
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl RobotsManager for InMemoryRobotsManager {
    async fn get(
        &self,
        _: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError> {
        let domain = url.atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        let cache = self.cache.read().await;
        let found = if let Some(found) = cache.get(&domain) {
            if let Some(max_age) = max_age {
                if (OffsetDateTime::now_utc() - found.retrieved_at()).le(max_age) {
                    Some(found.clone())
                } else {
                    drop(cache);
                    let mut cache = self.cache.write().await;
                    cache.remove(&domain);
                    None
                }
            } else {
                Some(found.clone())
            }
        } else {
            None
        };
        Ok(found)
    }

    async fn get_or_retrieve(
        &self,
        client: &Client,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError> {
        if let Some(found) = self.get(agent, url, max_age).await? {
            return Ok(found);
        }
        // Later used but cheaper than downloading and then recognizing invalidity for manager.
        let origin = url.atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        let result = client.get(&get_robots_url(&url.as_str())?).send().await?;
        let retrieved_at = OffsetDateTime::now_utc();
        let status_code = result.status();
        let result = result.bytes().await;

        let retrieved = if let Ok(result) = result {
            if status_code.is_client_error() || status_code.is_server_error() {
                CachedRobots::NoRobots {
                    retrieved_at,
                    status_code,
                }
            } else {
                let robot =
                    Robot::new(agent, result.as_ref()).map_err(RobotsError::InvalidRobotsTxt)?;
                CachedRobots::HasRobots {
                    robot,
                    retrieved_at,
                }
            }
        } else {
            CachedRobots::NoRobots {
                retrieved_at,
                status_code,
            }
        };

        let retrieved = Arc::new(retrieved);
        let mut cache = self.cache.write().await;
        let retrieved = if let Some(found) = cache.remove(&origin) {
            if found.retrieved_at() < retrieved.retrieved_at() {
                cache.insert(origin, retrieved.clone());
                retrieved
            } else {
                cache.insert(origin, found.clone());
                found
            }
        } else {
            cache.insert(origin, retrieved.clone());
            retrieved
        };
        drop(cache);
        Ok(retrieved)
    }
}

/// A manager for robots.txt, threadsafe, with some caching
#[derive(Debug)]
pub struct OffMemoryRobotsManager {
    db: Arc<DB>,
    cache: moka::future::Cache<AtraUrlOrigin, Arc<CachedRobots>>,
}

impl OffMemoryRobotsManager {
    declare_column_families! {
        self.db => cf_handle(ROBOTS_TXT_DB_CF)
    }

    /// Panics if the [Self::COLUMN_FAMILY] is not configured!
    pub fn new(db: Arc<DB>, cache_size: NonZeroUsize) -> Result<Self, RobotsError> {
        db_health_check!(db: [
            Self::ROBOTS_TXT_DB_CF => (
                if test robots_txt_cf_options
                else "The column family for the robots.txt is not configured!"
            )
        ]);

        Ok(Self {
            db,
            cache: moka::future::Cache::new(cache_size.get() as u64),
        })
    }

    async fn _set_cache(&self, key: AtraUrlOrigin, retrieved: CachedRobots) -> Arc<CachedRobots> {
        if let Some(associated) = self.cache.get(&key).await {
            if retrieved.retrieved_at() < associated.retrieved_at() {
                return associated;
            }
        };
        let new = Arc::new(retrieved);
        self.cache.insert(key, new.clone()).await;
        new
    }

    async fn _get_cached(
        &self,
        key: &AtraUrlOrigin,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
    ) -> Option<Arc<CachedRobots>> {
        if let Some(found) = self.cache.get(&key).await {
            log::trace!("Robots-Cache-Hit: {:?}", key);
            if let Some(max_age) = max_age {
                if (now - found.retrieved_at()).le(max_age) {
                    let found = found.clone();
                    return Some(found);
                }
            } else {
                return Some(found);
            }
        }
        None
    }

    async fn _get_db(
        &self,
        agent: &str,
        key: &AtraUrlOrigin,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
    ) -> Result<Option<CachedRobots>, RobotsError> {
        let cf = self.cf_handle();
        self._get_db0(agent, key, now, max_age, &cf)
    }

    async fn _get_or_retrieve(
        &self,
        client: &Client,
        agent: &str,
        key: &AtraUrlOrigin,
        url: &UrlWithDepth,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
    ) -> Result<CachedRobots, RobotsError> {
        if let Some(found) = self._get_db0(agent, key, now, max_age, &self.cf_handle())? {
            return Ok(found);
        }

        let result = client.get(&get_robots_url(&url.as_str())?).send().await?;
        let retrieved_at = OffsetDateTime::now_utc();
        let status_code = result.status();

        if status_code.is_client_error() || status_code.is_server_error() {
            return Ok(CachedRobots::NoRobots {
                retrieved_at,
                status_code,
            });
        }

        let result = result.bytes().await;

        let result = match result {
            Ok(result) => result,
            _ => {
                return Ok(CachedRobots::NoRobots {
                    retrieved_at,
                    status_code,
                })
            }
        };

        let bytes = BytesWithAge {
            bytes: result.as_ref(),
            retrieved_at,
        };
        let value = bincode::serialize(&bytes)?;
        self.db
            .put_cf(&self.cf_handle(), key.as_bytes(), &value)
            .enrich_with_entry(Self::ROBOTS_TXT_DB_CF, Write, key.as_bytes(), &value)?;
        drop(value);
        yield_now().await;

        let robot = Robot::new(agent, result.as_ref()).map_err(RobotsError::InvalidRobotsTxt)?;
        return Ok(CachedRobots::HasRobots {
            robot,
            retrieved_at,
        });
    }

    fn _get_db0<'a>(
        &self,
        agent: &str,
        key: &AtraUrlOrigin,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
        cf: &'a Arc<BoundColumnFamily<'a>>,
    ) -> Result<Option<CachedRobots>, RobotsError> {
        let result = self
            .db
            .get_pinned_cf(cf, key.as_bytes())
            .enrich_without_entry(Self::ROBOTS_TXT_DB_CF, Read, key.as_bytes())?;
        if let Some(result) = result {
            let found: BytesWithAge = bincode::deserialize(&result)?;
            if let Some(max_age) = max_age {
                if (now - found.retrieved_at).le(max_age) {
                    let robot =
                        Robot::new(agent, found.bytes).map_err(RobotsError::InvalidRobotsTxt)?;
                    return Ok(Some(CachedRobots::HasRobots {
                        robot,
                        retrieved_at: found.retrieved_at,
                    }));
                } else {
                    drop(result);
                    self.db.delete_cf(cf, key.as_bytes()).enrich_without_entry(
                        Self::ROBOTS_TXT_DB_CF,
                        Delete,
                        key.as_bytes(),
                    )?;
                }
            } else {
                let robot =
                    Robot::new(agent, found.bytes).map_err(RobotsError::InvalidRobotsTxt)?;
                return Ok(Some(CachedRobots::HasRobots {
                    robot,
                    retrieved_at: found.retrieved_at,
                }));
            }
        }
        Ok(None)
    }

    // async fn clear_cache(&self) {
    //     self.cache.lock().await.clear()
    // }
}

impl RobotsManager for OffMemoryRobotsManager {
    /// A faster version of `get_or_retrieve` where no client is needed.
    /// Returns None if there is no robots.txt in any cache level.
    async fn get(
        &self,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError> {
        let now = OffsetDateTime::now_utc();
        let key = url.url().atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        let found = self._get_cached(&key, now.clone(), max_age.clone()).await;
        if found.is_some() {
            return Ok(found);
        }
        let found = self._get_db(agent, &key, now, max_age.clone()).await?;
        if let Some(found) = found {
            Ok(Some(self._set_cache(key, found).await))
        } else {
            Ok(None)
        }
    }

    /// Uses a mutex internally, therefore you should cache the returned value in your task.
    /// If nothing is in any cache it downloads the robots.txt with the client.
    async fn get_or_retrieve(
        &self,
        client: &Client,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError> {
        let now = OffsetDateTime::now_utc();
        let key = url.url().atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        match self._get_cached(&key, now.clone(), max_age).await {
            Some(found) => return Ok(found),
            _ => {}
        }
        let retrieved = self
            ._get_or_retrieve(client, agent, &key, url, now, max_age)
            .await?;
        Ok(self._set_cache(key, retrieved).await)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BytesWithAge<'a> {
    bytes: &'a [u8],
    retrieved_at: OffsetDateTime,
}

#[cfg(test)]
mod test {
    use crate::client::ClientBuilder;
    use crate::config::system::DEFAULT_CACHE_SIZE_ROBOTS;
    use crate::database::{destroy_db, open_db};
    use crate::robots::{OffMemoryRobotsManager, RobotsManager};
    use crate::url::UrlWithDepth;
    use rocksdb::Options;
    use scopeguard::defer;
    use std::sync::Arc;
    use std::time::Instant;

    #[tokio::test]
    async fn can_manage_a_robots_txt() {
        defer! {
            let _ = destroy_db("test.db1");
        }

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let db = Arc::new(open_db("test.db1").expect("open or create the db"));

        let manager =
            OffMemoryRobotsManager::new(db, DEFAULT_CACHE_SIZE_ROBOTS).expect("create the manager");

        const USER_AGENT: &'static str = "test_crawl";

        let target_url = UrlWithDepth::from_seed("https://choosealicense.com/").unwrap();

        let client = ClientBuilder::new(
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap(),
        )
        .build();

        let now = Instant::now();

        let robots = manager
            .get_or_retrieve(&client, USER_AGENT, &target_url, None)
            .await
            .unwrap();
        println!("without_cache: {:?}", Instant::now() - now);
        println!("{:?}", robots);
        println!("{}", robots.retrieved_at());

        let now = Instant::now();

        let robots = manager
            .get_or_retrieve(&client, USER_AGENT, &target_url, None)
            .await
            .unwrap();
        println!("with_cache: {:?}", Instant::now() - now);
        println!("{:?}", robots);
    }

    #[tokio::test]
    async fn can_manage_a_robots_txt_in_memory() {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let manager = crate::robots::InMemoryRobotsManager::new();

        const USER_AGENT: &'static str = "test_crawl";

        let target_url = UrlWithDepth::from_seed("https://choosealicense.com/").unwrap();

        let client = ClientBuilder::new(
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .unwrap(),
        )
        .build();

        let now = Instant::now();

        let robots = manager
            .get_or_retrieve(&client, USER_AGENT, &target_url, None)
            .await
            .unwrap();
        println!("without_cache: {:?}", Instant::now() - now);
        println!("{:?}", robots);
        println!("{}", robots.retrieved_at());

        let now = Instant::now();

        let robots = manager
            .get_or_retrieve(&client, USER_AGENT, &target_url, None)
            .await
            .unwrap();
        println!("with_cache: {:?}", Instant::now() - now);
        println!("{:?}", robots);
    }
}
