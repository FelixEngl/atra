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

use std::fs::File;
use std::io::{BufReader};
use std::path::Path;
use camino::Utf8Path;
use ini::{Ini};
use crate::core::config::crawl::CrawlConfig;
use crate::core::config::paths::PathsConfig;
use crate::core::config::session::SessionConfig;
use crate::core::config::SystemConfig;
use crate::core::ini_ext::{FromIni};

/// A collection of all config used in a crawl.
/// Can be shared across threads
#[derive(Debug, Default, Clone)]
pub struct Configs {
    pub system: SystemConfig,
    pub paths: PathsConfig,
    pub crawl: CrawlConfig,
    pub session: SessionConfig,
}

impl Configs {
    #[cfg(test)] pub fn new(system: SystemConfig, paths: PathsConfig, crawl: CrawlConfig, session: SessionConfig) -> Self {
        Self {
            system,
            paths,
            crawl,
            session,
        }
    }
    #[inline]
    pub fn paths(&self) -> &PathsConfig {
        &self.paths
    }
    #[inline]
    pub fn system(&self) -> &SystemConfig {
        &self.system
    }
    #[inline]
    pub fn crawl(&self) -> &CrawlConfig { &self.crawl }
    #[inline]
    pub fn session(&self) -> &SessionConfig { &self.session }

    pub fn load_from_config_folder<P: AsRef<Path>>(folder: P) -> Configs {
        let path = folder.as_ref();
        let system_config_path = path.join("atra.ini");


        let (system, paths, session): (SystemConfig, PathsConfig, SessionConfig) = if system_config_path.exists() {
            let ini = Ini::load_from_file(system_config_path).unwrap();
            (
                SystemConfig::from_ini(&ini),
                PathsConfig::from_ini(&ini),
                SessionConfig::default(),
            )
        } else {
            Default::default()
        };

        let crawl_config_path = path.join("crawl.yaml");
        let crawl: CrawlConfig = if crawl_config_path.exists() {
            serde_yaml::from_reader(BufReader::new(File::options().read(true).open(crawl_config_path).expect("Was not able to load the crawl config properly."))).expect("Was not able to read config!")
        } else {
            CrawlConfig::default()
        };

        Self {
            system,
            paths,
            session,
            crawl
        }
    }

    pub fn discover_or_default() -> Self {
        let ini_path = Utf8Path::new("atra.ini");

        let (system, paths, session) = if ini_path.exists() {
            match ini::Ini::load_from_file(ini_path) {
                Ok(ini) => {
                    (
                        SystemConfig::from_ini(&ini),
                        PathsConfig::from_ini(&ini),
                        SessionConfig::from_ini(&ini)
                    )
                }
                Err(err) => {
                    log::error!("Failed to load ini, using fallback: {err}");
                    Default::default()
                }
            }
        } else {
            Default::default()
        };

        let mut crawl_path = Utf8Path::new("crawl.yaml");
        if !crawl_path.exists() {
            crawl_path = Utf8Path::new("crawl.yml");
        }

        let crawl = if crawl_path.exists() {
            match File::options().read(true).open(crawl_path) {
                Ok(file) => {
                    match serde_yaml::from_reader::<_, CrawlConfig>(file) {
                        Ok(cfg) => {
                            cfg
                        }
                        Err(err) => {
                            log::error!("Failed to deserialize the config with {err}");
                            CrawlConfig::default()
                        }
                    }
                }
                Err(err) => {
                    log::error!("Failed to deserialize the config with {err}");
                    CrawlConfig::default()
                }
            }
        } else {
            CrawlConfig::default()
        };

        Configs {
            system,
            paths,
            session,
            crawl
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::num::{NonZeroU64, NonZeroUsize};
    use std::path::Path;
    use case_insensitive_string::CaseInsensitiveString;
    use ini::Ini;
    use reqwest::header::{HeaderMap, HeaderValue};
    use time::Duration;
    use ubyte::ToByteUnit;
    use crate::core::config::{BudgetSettings, Configs};
    use crate::core::config::crawl::{CookieSettings, RedirectPolicy, UserAgent};
    use crate::core::extraction::extractor::{Extractor};
    use crate::core::ini_ext::IntoIni;

    pub fn export<P: AsRef<Path>>(config: &Configs, path: P) {
        let path = path.as_ref();
        let system_config_path = path.join("atra.ini");
        let mut ini = Ini::new();
        config.paths.insert_into(&mut ini);
        config.system.insert_into(&mut ini);
        config.session.insert_into(&mut ini);
        ini.write_to_file(system_config_path).unwrap();
        let crawl_config_path = path.join("crawl.yaml");
        let result = serde_yaml::to_string(config.crawl()).unwrap();
        let mut data = BufWriter::new(File::options().write(true).create(true).open(crawl_config_path).unwrap());
        data.write(result.as_bytes()).unwrap();
    }

    #[test]
    fn full_config(){
        let mut cfg = Configs::default();
        cfg.paths.db_dir_name = Some("<example>".to_string());
        cfg.paths.db_dir_name = Some("<example>".to_string());
        cfg.paths.queue_file_name = Some("<example>".to_string());
        cfg.paths.blacklist_name = Some("<example>".to_string());
        cfg.paths.big_file_dir_name = Some("<example>".to_string());
        cfg.paths.web_graph_file_name = Some("<example>".to_string());
        cfg.paths.root_folder = "<example>".to_string();

        cfg.session.service_name = "<example>".to_string();
        cfg.session.crawl_job_id = 1;
        cfg.session.collection_name = "<example>".to_string();

        cfg.system.web_graph_cache_size = NonZeroUsize::new(50_000).unwrap();
        cfg.system.max_file_size_in_memory = 100.megabytes().as_u64();
        cfg.system.robots_cache_size = NonZeroUsize::new(50).unwrap();

        cfg.crawl.user_agent = UserAgent::Custom("<Example Agent>".to_string());
        cfg.crawl.respect_robots_txt = true;
        cfg.crawl.respect_nofollow = true;
        cfg.crawl.crawl_embedded_data = true;
        cfg.crawl.crawl_javascript = true;
        cfg.crawl.crawl_onclick_by_heuristic = true;
        cfg.crawl.max_file_size = NonZeroU64::new(1.gigabytes().as_u64());
        cfg.crawl.max_robots_age = Some(Duration::days(7));
        cfg.crawl.ignore_sitemap = false;
        cfg.crawl.subdomains = true;
        cfg.crawl.cache = true;
        cfg.crawl.use_cookies = true;

        let mut cookies = CookieSettings::default();
        cookies.default = Some("cookie text".to_string());
        let mut cookie_hash = HashMap::new();
        cookie_hash.insert(CaseInsensitiveString::new(b"example.com"), "cookie text".to_string());
        cookie_hash.insert(CaseInsensitiveString::new(b"google.de"), "cookie text".to_string());
        cookies.per_domain = Some(cookie_hash);
        cfg.crawl.cookies = Some(cookies);

        let mut header_map = HeaderMap::new();
        header_map.insert(reqwest::header::CONTENT_LENGTH, HeaderValue::from_static("150000"));
        header_map.insert(reqwest::header::CONTENT_TYPE, HeaderValue::from_static("html/text"));
        cfg.crawl.headers = Some(header_map);

        cfg.crawl.proxies = Some(vec!["www.example.com:2020".to_string(), "www.example.com:2021".to_string()]);
        cfg.crawl.tld = true;

        cfg.crawl.delay = Some(Duration::milliseconds(100));
        cfg.crawl.budget.default = BudgetSettings::Normal {
            depth: 5,
            recrawl_interval: Some(Duration::days(7)),
            request_timeout: Some(Duration::seconds(10)),
            depth_on_website: 5
        };

        let mut hash = HashMap::new();
        hash.insert(CaseInsensitiveString::new(b"example.com"), BudgetSettings::Absolute {
            depth: 3,
            recrawl_interval: Some(Duration::days(7)),
            request_timeout: Some(Duration::seconds(10)),
        });

        hash.insert(CaseInsensitiveString::new(b"google.de"), BudgetSettings::SeedOnly {
            depth_on_website: 2,
            recrawl_interval: Some(Duration::days(7)),
            request_timeout: Some(Duration::seconds(10)),
        });

        cfg.crawl.budget.per_host = Some(hash);

        cfg.crawl.max_queue_age = 18;

        cfg.crawl.redirect_limit = 20;

        cfg.crawl.redirect_policy = RedirectPolicy::Strict;

        cfg.crawl.accept_invalid_certs = true;
        cfg.crawl.extractors = Extractor::default();

        cfg.crawl.decode_big_files_up_to = Some(150_000);

        export(&cfg, "./configs")
    }
}
