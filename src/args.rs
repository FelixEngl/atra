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
use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::Path;
use clap::{Parser, Subcommand};
use crate::core::config::crawl::{CookieSettings, RedirectPolicy, UserAgent};
use std::str::FromStr;
use case_insensitive_string::CaseInsensitiveString;
use ini::Ini;
use reqwest::header::{HeaderMap, HeaderValue};
use time::Duration;
use ubyte::ToByteUnit;
use crate::application::ApplicationMode;
use crate::core::config::{BudgetSettings, Configs};
use crate::core::extraction::extractor::{Extractor, SubExtractor};
use crate::core::ini_ext::IntoIni;
use crate::core::seeds::seed_definition::SeedDefinition;

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
/// Welcome to Atra
pub struct AtraArgs {
    /// A command to initialize exemplary configs
    #[arg(long)]
    pub generate_example_config: bool,

    /// The mode of Atra
    #[command(subcommand)]
    pub mode: Option<RunMode>,
}




#[derive(Subcommand, Debug)]
pub enum RunMode {
    /// Single mode allows to crawls on a single seed without leaving the domain.
    SINGLE {
        /// The name of the crawl
        #[arg(short, long)]
        session_name: Option<String>,
        /// What is the name of the agent?
        #[arg(short, long, value_parser = UserAgent::from_str, default_value_t = UserAgent::Default)]
        agent: UserAgent,
        /// How deep do you want to crawl on the domain, starting from the seed?
        #[arg(short, long)]
        depth: u64,
        /// Sets the crawl mode to absolute, crawling everything until reaching in every direction, not only the domain.
        #[arg(long)]
        absolute: bool,
        /// Timeout in seconds, if not set never time out.
        #[arg(short, long)]
        timeout: Option<f64>,
        /// The log level of Atra
        #[arg(long, default_value_t = log::LevelFilter::Info)]
        log_level: log::LevelFilter,
        /// Log to file
        #[arg(long)]
        log_to_file: bool,
        /// The seed url to be crawled.
        seeds: SeedDefinition
    },
    /// Crawl multiple seeds
    MULTI {
        /// The name of the crawl
        #[arg(short, long)]
        session_name: Option<String>,
        /// The number of threads used by this application.
        #[arg(short, long)]
        threads: Option<usize>,
        /// The folder containing the required configs.
        #[arg(short, long)]
        config: Option<String>,
        /// overrides the log level from the config.
        #[arg(long)]
        override_log_level: Option<log::LevelFilter>,
        /// Log to file
        #[arg(long)]
        log_to_file: bool,
        /// Seed to be crawled
        seeds: SeedDefinition
    },
    // CLUSTER,
}


#[derive(Debug)]
pub enum ConsumedArgs {
    RunConfig(ApplicationMode, SeedDefinition, Configs),
    Nothing
}



/// Consumes the args and returns everything necessary to execute Atra
pub(crate) fn consume_args(args: AtraArgs) -> ConsumedArgs {

    if let Some(mode) = args.mode {
        match mode {
            RunMode::SINGLE { session_name, absolute, agent, seeds, depth, timeout, log_level, log_to_file} => {
                let mut configs = Configs::discover_or_default();

                configs.paths.root_folder = configs.paths.root_path().join(
                    format!("single_{}_{}",
                            data_encoding::BASE32.encode(&time::OffsetDateTime::now_utc().unix_timestamp_nanos().to_be_bytes()),
                            data_encoding::BASE32.encode(&rand::random::<u64>().to_be_bytes()),
                    )
                ).to_string();

                configs.crawl.user_agent = agent;

                if let Some(session_name) = session_name {
                    configs.session.service_name = session_name
                }

                configs.crawl.budget.default = if absolute {
                    BudgetSettings::Absolute {
                        depth,
                        recrawl_interval: None,
                        request_timeout: timeout.map(|value| Duration::saturating_seconds_f64(value))
                    }
                } else {
                    BudgetSettings::SeedOnly {
                        depth_on_website: depth,
                        recrawl_interval: None,
                        request_timeout: timeout.map(|value| Duration::saturating_seconds_f64(value))
                    }
                };

                configs.system.log_level = log_level;

                configs.system.log_to_file = log_to_file;

                ConsumedArgs::RunConfig(
                    ApplicationMode::Single,
                    seeds,
                    configs
                )
            }
            RunMode::MULTI { session_name, seeds, config: configs_folder, threads, override_log_level: log_level, log_to_file } => {
                let mut configs = match configs_folder {
                    None => {Configs::discover_or_default()}
                    Some(path) => {Configs::load_from_config_folder(path)}
                };

                configs.paths.root_folder = configs.paths.root_path().join(
                    format!("multi_{}_{}",
                            data_encoding::BASE32.encode(&time::OffsetDateTime::now_utc().unix_timestamp_nanos().to_be_bytes()),
                            data_encoding::BASE32.encode(&rand::random::<u64>().to_be_bytes()),
                    )
                ).to_string();

                configs.system.log_to_file = log_to_file;

                if let Some(session_name) = session_name {
                    configs.session.service_name = session_name
                }

                if let Some(log_level) = log_level {
                    configs.system.log_level = log_level;
                }

                ConsumedArgs::RunConfig(
                    ApplicationMode::Multi(threads.map(|value| NonZeroUsize::new(value)).flatten()),
                    seeds,
                    configs
                )
            }
        }
    } else {
        if args.generate_example_config {
            let mut cfg = Configs::default();
            cfg.paths.root_folder = "atra_data".to_string();
            cfg.paths.db_dir_name = Some(cfg.paths.dir_database_name().to_string());
            cfg.paths.queue_file_name = Some(cfg.paths.file_queue_name().to_string());
            cfg.paths.blacklist_name = Some(cfg.paths.file_blacklist_name().to_string());
            cfg.paths.big_file_dir_name = Some(cfg.paths.dir_big_files_name().to_string());
            cfg.paths.web_graph_file_name = Some(cfg.paths.file_web_graph_name().to_string());

            cfg.session.crawl_job_id = 0;
            cfg.session.service_name = "MyServiceName".to_string();
            cfg.session.collection_name = "MyCollection".to_string();

            cfg.system.web_graph_cache_size = NonZeroUsize::new(20_000).unwrap();
            cfg.system.max_file_size_in_memory = 100.megabytes().as_u64();
            cfg.system.robots_cache_size = NonZeroUsize::new(50).unwrap();

            cfg.crawl.user_agent = UserAgent::Custom("Atra/TestCrawl/<your email>".to_string());
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
            cookies.default = Some("Cookie String".to_string());
            let mut cookie_hash = HashMap::new();
            cookie_hash.insert(CaseInsensitiveString::new(b"example.com"), "Cookie String".to_string());
            cookie_hash.insert(CaseInsensitiveString::new(b"example.de"), "Cookie String".to_string());
            cookies.per_domain = Some(cookie_hash);
            cfg.crawl.cookies = Some(cookies);

            let mut header_map = HeaderMap::new();
            header_map.insert(reqwest::header::CONTENT_LENGTH, HeaderValue::from_static("10_000"));
            header_map.insert(reqwest::header::CONTENT_TYPE, HeaderValue::from_static("html/text"));
            cfg.crawl.headers = Some(header_map);

            cfg.crawl.proxies = Some(vec!["www.example.com:2020".to_string(), "www.example.com:2021".to_string()]);
            cfg.crawl.tld = true;

            cfg.crawl.delay = Some(Duration::milliseconds(100));
            cfg.crawl.budget.default = BudgetSettings::Normal {
                depth: 3,
                depth_on_website: 5,
                recrawl_interval: Some(Duration::days(7)),
                request_timeout: Some(Duration::seconds(10)),
            };

            let mut hash = HashMap::new();
            hash.insert(CaseInsensitiveString::new(b"example.com"), BudgetSettings::Normal {
                depth: 3,
                depth_on_website: 5,
                recrawl_interval: Some(Duration::days(7)),
                request_timeout: Some(Duration::seconds(10)),
            });

            hash.insert(CaseInsensitiveString::new(b"example.de"), BudgetSettings::Absolute {
                depth: 3,
                recrawl_interval: Some(Duration::days(7)),
                request_timeout: Some(Duration::seconds(10)),
            });

            hash.insert(CaseInsensitiveString::new(b"example.org"), BudgetSettings::SeedOnly {
                depth_on_website: 5,
                recrawl_interval: Some(Duration::days(7)),
                request_timeout: Some(Duration::seconds(10)),
            });

            cfg.crawl.budget.per_host = Some(hash);

            cfg.crawl.max_queue_age = 18;

            cfg.crawl.redirect_limit = 20;

            cfg.crawl.redirect_policy = RedirectPolicy::Strict;

            cfg.crawl.accept_invalid_certs = true;
            cfg.crawl.extractors = Extractor(SubExtractor::ALL_ENTRIES.to_vec());

            cfg.crawl.decode_big_files_up_to = Some(200.megabytes().as_u64());

            fn export<P: AsRef<Path>>(config: &Configs, path: P) {
                let path = path.as_ref();
                let system_config_path = path.join("atra.example.ini");
                let mut ini = Ini::new();
                config.paths.insert_into(&mut ini);
                config.system.insert_into(&mut ini);
                config.session.insert_into(&mut ini);
                ini.write_to_file(system_config_path).unwrap();
                let crawl_config_path = path.join("crawl.example.yaml");
                let result = serde_yaml::to_string(config.crawl()).unwrap();
                let mut data = BufWriter::new(File::options().write(true).truncate(true).create(true).open(crawl_config_path).unwrap());
                data.write(result.as_bytes()).unwrap();

                File::options().write(true).create(true).truncate(true).open(path.join("ReadMe.txt")).unwrap().write_all(
                    b"Every value in the example configs can be deleted if not needed.\n\
                    Rename the files to atra.ini and crawl.yaml to use them as default configs."
                ).unwrap();
            }

            export(&cfg, ".");
            ConsumedArgs::Nothing
        } else {
            ConsumedArgs::Nothing
        }
    }


}