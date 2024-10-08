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

use crate::client::traits::{AtraClient, AtraResponse};
use crate::database::DBActionType::{Delete, Read, Write};
use crate::database::RawDatabaseError;
use crate::robots::{CachedRobots, RobotsError, RobotsManager};
use crate::url::UrlWithDepth;
use crate::url::{AtraOriginProvider, AtraUrlOrigin};
use crate::{db_health_check, declare_column_families};
use rocksdb::{BoundColumnFamily, DB};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::num::NonZeroUsize;
use std::sync::Arc;
use texting_robots::{get_robots_url, Robot};
use time::{Duration, OffsetDateTime};
use tokio::task::yield_now;

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
    pub fn new(db: Arc<DB>, cache_size: NonZeroUsize) -> Self {
        db_health_check!(db: [
            Self::ROBOTS_TXT_DB_CF => (
                if test robots_txt_cf_options
                else "The column family for the robots.txt is not configured!"
            )
        ]);

        Self {
            db,
            cache: moka::future::Cache::new(cache_size.get() as u64),
        }
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

    async fn _get_db<E: Error>(
        &self,
        agent: &str,
        key: &AtraUrlOrigin,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
    ) -> Result<Option<CachedRobots>, RobotsError<E>> {
        let cf = self.cf_handle();
        self._get_db0(agent, key, now, max_age, &cf)
    }

    async fn _get_or_retrieve<C: AtraClient>(
        &self,
        client: &C,
        agent: &str,
        key: &AtraUrlOrigin,
        url: &UrlWithDepth,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
    ) -> Result<CachedRobots, RobotsError<C::Error>> {
        if let Some(found) = self._get_db0(agent, key, now, max_age, &self.cf_handle())? {
            return Ok(found);
        }

        let result = client
            .get(&get_robots_url(&url.try_as_str())?)
            .await
            .map_err(RobotsError::ClientWasNotAbleToSend)?;
        let retrieved_at = OffsetDateTime::now_utc();
        let status_code = result.status();

        if status_code.is_client_error() || status_code.is_server_error() {
            return Ok(CachedRobots::NoRobots {
                retrieved_at,
                _status_code: status_code,
            });
        }

        let result = result.bytes().await;

        let result = match result {
            Ok(result) => result,
            _ => {
                return Ok(CachedRobots::NoRobots {
                    retrieved_at,
                    _status_code: status_code,
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

    fn _get_db0<'a, E: Error>(
        &self,
        agent: &str,
        key: &AtraUrlOrigin,
        now: OffsetDateTime,
        max_age: Option<&Duration>,
        cf: &'a Arc<BoundColumnFamily<'a>>,
    ) -> Result<Option<CachedRobots>, RobotsError<E>> {
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
    async fn get<E: Error>(
        &self,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError<E>> {
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
    async fn get_or_retrieve<C: AtraClient>(
        &self,
        client: &C,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError<C::Error>> {
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
    // use crate::config::system::DEFAULT_CACHE_SIZE_ROBOTS;
    // use crate::database::{destroy_db, open_db};
    // use crate::robots::{OffMemoryRobotsManager, RobotsManager};
    // use crate::url::UrlWithDepth;
    // use rocksdb::Options;
    // use scopeguard::defer;
    // use std::sync::Arc;
    // use std::time::Instant;
    // #[tokio::test]
    // async fn can_manage_a_robots_txt() {
    //     defer! {
    //         let _ = destroy_db("test.db1");
    //     }
    //
    //     let mut opts = Options::default();
    //     opts.create_if_missing(true);
    //     opts.create_missing_column_families(true);
    //
    //     let db = Arc::new(open_db("test.db1").expect("open or create the db"));
    //
    //     let manager =
    //         OffMemoryRobotsManager::new(db, DEFAULT_CACHE_SIZE_ROBOTS).expect("create the manager");
    //
    //     const USER_AGENT: &'static str = "test_crawl";
    //
    //     let target_url = UrlWithDepth::from_seed("https://choosealicense.com/").unwrap();
    //
    //     let client = ClientBuilder::new(
    //         reqwest::Client::builder()
    //             .user_agent(USER_AGENT)
    //             .build()
    //             .unwrap(),
    //     )
    //     .build();
    //
    //     let now = Instant::now();
    //
    //     let robots = manager
    //         .get_or_retrieve(&client, USER_AGENT, &target_url, None)
    //         .await
    //         .unwrap();
    //     println!("without_cache: {:?}", Instant::now() - now);
    //     println!("{:?}", robots);
    //     println!("{}", robots.retrieved_at());
    //
    //     let now = Instant::now();
    //
    //     let robots = manager
    //         .get_or_retrieve(&client, USER_AGENT, &target_url, None)
    //         .await
    //         .unwrap();
    //     println!("with_cache: {:?}", Instant::now() - now);
    //     println!("{:?}", robots);
    // }
    //
    // #[tokio::test]
    // async fn can_manage_a_robots_txt_in_memory() {
    //     let mut opts = Options::default();
    //     opts.create_if_missing(true);
    //     opts.create_missing_column_families(true);
    //
    //     let manager = crate::robots::InMemoryRobotsManager::new();
    //
    //     const USER_AGENT: &'static str = "test_crawl";
    //
    //     let target_url = UrlWithDepth::from_seed("https://choosealicense.com/").unwrap();
    //
    //     let client = ClientBuilder::new(
    //         reqwest::Client::builder()
    //             .user_agent(USER_AGENT)
    //             .build()
    //             .unwrap(),
    //     )
    //     .build();
    //
    //     let now = Instant::now();
    //
    //     let robots = manager
    //         .get_or_retrieve(&client, USER_AGENT, &target_url, None)
    //         .await
    //         .unwrap();
    //     println!("without_cache: {:?}", Instant::now() - now);
    //     println!("{:?}", robots);
    //     println!("{}", robots.retrieved_at());
    //
    //     let now = Instant::now();
    //
    //     let robots = manager
    //         .get_or_retrieve(&client, USER_AGENT, &target_url, None)
    //         .await
    //         .unwrap();
    //     println!("with_cache: {:?}", Instant::now() - now);
    //     println!("{:?}", robots);
    // }
}
