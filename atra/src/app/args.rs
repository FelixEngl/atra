// Copyright 2024. Felix Engl
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

use std::fs::File;
use std::io::{BufWriter, Write};
use std::num::NonZeroUsize;
use std::str::FromStr;

use clap::{Parser, Subcommand};
use time::Duration;

use crate::app::atra::ApplicationMode;
use crate::app::constants::{create_example_config, ATRA_LOGO, ATRA_WELCOME};
use crate::config::crawl::UserAgent;
use crate::config::{BudgetSetting, Configs};
use crate::seed::SeedDefinition;

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
        #[arg(
            short, long, value_parser = UserAgent::from_str, default_value_t = UserAgent::Default
        )]
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
        seeds: SeedDefinition,
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
        seeds: SeedDefinition,
    },
    // CLUSTER,
    WELCOME,
}

#[derive(Debug)]
pub enum ConsumedArgs {
    RunConfig(ApplicationMode, SeedDefinition, Configs),
    Nothing,
}

/// Consumes the args and returns everything necessary to execute Atra
pub(crate) fn consume_args(args: AtraArgs) -> ConsumedArgs {
    if let Some(mode) = args.mode {
        match mode {
            RunMode::SINGLE {
                session_name,
                absolute,
                agent,
                seeds,
                depth,
                timeout,
                log_level,
                log_to_file,
            } => {
                let mut configs =
                    Configs::discover_or_default().expect("Expected some kind of config!");

                configs.paths.root = configs.paths.root_path().join(format!(
                    "single_{}_{}",
                    data_encoding::BASE32.encode(
                        &time::OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes()
                    ),
                    data_encoding::BASE32.encode(&rand::random::<u64>().to_be_bytes()),
                ));

                configs.crawl.user_agent = agent;

                if let Some(session_name) = session_name {
                    configs.session.service = session_name
                }

                configs.crawl.budget.default = if absolute {
                    BudgetSetting::Absolute {
                        depth,
                        recrawl_interval: None,
                        request_timeout: timeout
                            .map(|value| Duration::saturating_seconds_f64(value)),
                    }
                } else {
                    BudgetSetting::SeedOnly {
                        depth_on_website: depth,
                        recrawl_interval: None,
                        request_timeout: timeout
                            .map(|value| Duration::saturating_seconds_f64(value)),
                    }
                };

                configs.system.log_level = log_level;

                configs.system.log_to_file = log_to_file;

                ConsumedArgs::RunConfig(ApplicationMode::Single, seeds, configs)
            }
            RunMode::MULTI {
                session_name,
                seeds,
                config: configs_folder,
                threads,
                override_log_level: log_level,
                log_to_file,
            } => {
                let mut configs = match configs_folder {
                    None => Configs::discover_or_default(),
                    Some(path) => Configs::load_from(path),
                }
                .expect("Expected some kind of config!");

                configs.paths.root = configs.paths.root_path().join(format!(
                    "multi_{}_{}",
                    data_encoding::BASE32.encode(
                        &time::OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes()
                    ),
                    data_encoding::BASE32.encode(&rand::random::<u64>().to_be_bytes()),
                ));

                configs.system.log_to_file = log_to_file;

                if let Some(session_name) = session_name {
                    configs.session.service = session_name
                }

                if let Some(log_level) = log_level {
                    configs.system.log_level = log_level;
                }

                ConsumedArgs::RunConfig(
                    ApplicationMode::Multi(threads.map(|value| NonZeroUsize::new(value)).flatten()),
                    seeds,
                    configs,
                )
            }
            RunMode::WELCOME => {
                println!("{}\n\n{}\n", ATRA_WELCOME, ATRA_LOGO);
                ConsumedArgs::Nothing
            }
        }
    } else {
        if args.generate_example_config {
            let cfg = create_example_config();
            let root = cfg.paths.root_path();
            match File::open(root.join("example_config.json")) {
                Ok(file) => match serde_json::to_writer(BufWriter::new(file), &cfg) {
                    Ok(_) => {}
                    Err(err) => {
                        println!("Failed to create the example file: {err}")
                    }
                },
                Err(err) => {
                    println!("Failed to create the example file: {err}")
                }
            }
            ConsumedArgs::Nothing
        } else {
            ConsumedArgs::Nothing
        }
    }
}
