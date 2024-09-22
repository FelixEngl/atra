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

mod error;
mod instruction;

use crate::app::args::RunMode;
use crate::app::config::{discover, discover_or_default, try_load_from_path};
use crate::app::constants::{create_example_config, ATRA_LOGO, ATRA_WELCOME};
use crate::app::view::view;
use crate::app::{ApplicationMode, AtraArgs};
use crate::config::{BudgetSetting, Config};
use crate::contexts::local::LocalContext;
use camino::Utf8PathBuf;
pub use error::*;
pub use instruction::*;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::num::NonZeroUsize;
use time::Duration;

/// Consumes the args and returns everything necessary to execute Atra
pub(crate) fn prepare_instruction(args: AtraArgs) -> Result<Instruction, InstructionError> {
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
                delay,
            } => {
                let mut config = discover_or_default().unwrap_or_default();

                log::info!(
                    "Session Info: {} - {} - {}",
                    config.session.service,
                    config.session.collection,
                    config.session.crawl_job_id
                );

                config.paths.root = config.paths.root_path().join(format!(
                    "single_{}_{}",
                    data_encoding::BASE64URL.encode(
                        &time::OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes()
                    ),
                    data_encoding::BASE64URL.encode(&rand::random::<u64>().to_be_bytes()),
                ));

                config.crawl.user_agent = agent;

                if let Some(session_name) = session_name {
                    config.session.service = session_name
                }

                config.crawl.budget.default = if absolute {
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

                config.crawl.delay = delay
                    .map(|value| std::time::Duration::from_millis(value).try_into().ok())
                    .flatten();

                config.system.log_level = log_level;

                config.system.log_to_file = log_to_file;

                Ok(Instruction::RunInstruction(RunInstruction {
                    mode: ApplicationMode::Single,
                    config,
                    seeds: Some(seeds),
                    recover_mode: false,
                }))
            }
            RunMode::MULTI {
                session_name,
                seeds,
                config: configs_folder,
                threads,
                override_log_level: log_level,
                log_to_file,
            } => {
                let mut config = match configs_folder {
                    None => discover(),
                    Some(path) => try_load_from_path(path),
                }?;

                log::info!(
                    "Session Info: {} - {} - {}",
                    config.session.service,
                    config.session.collection,
                    config.session.crawl_job_id
                );

                config.paths.root = config.paths.root_path().join(format!(
                    "multi_{}_{}",
                    data_encoding::BASE64URL.encode(
                        &time::OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes()
                    ),
                    data_encoding::BASE64URL.encode(&rand::random::<u64>().to_be_bytes()),
                ));

                config.system.log_to_file = log_to_file;

                if let Some(session_name) = session_name {
                    config.session.service = session_name
                }

                if let Some(log_level) = log_level {
                    config.system.log_level = log_level;
                }

                Ok(Instruction::RunInstruction(RunInstruction {
                    mode: ApplicationMode::Multi(
                        threads.map(|value| NonZeroUsize::new(value)).flatten(),
                    ),
                    config,
                    seeds: Some(seeds),
                    recover_mode: false,
                }))
            }
            RunMode::INIT => {
                println!("{}\n\n{}\n\n", ATRA_WELCOME, ATRA_LOGO);
                println!("Start creating the default config.");
                let cfg = Config::default();
                let root = cfg.paths.root_path();
                std::fs::create_dir_all(root)?;
                let path = root.join("config.json");
                if path.exists() {
                    println!("The default config already exists in {path}.\nDelete is before regenerating.")
                } else {
                    match File::options().create(true).write(true).open(&path) {
                        Ok(file) => {
                            match serde_json::to_writer_pretty(BufWriter::new(file), &cfg) {
                                Ok(_) => {}
                                Err(err) => {
                                    println!("Failed to create the example file: {err}")
                                }
                            }
                        }
                        Err(err) => {
                            println!("Failed to create the example file: {err}")
                        }
                    }
                    println!("Created the default config at {}.", path);
                }

                Ok(Instruction::Nothing)
            }
            RunMode::RECOVER {
                threads,
                log_to_file,
                path,
            } => {
                let path = Utf8PathBuf::from(path);

                let mut config = if path.is_dir() {
                    let mut cfg: Config = try_load_from_path(&path)?;
                    cfg.paths.root = path;
                    cfg
                } else if path.is_file() {
                    let file = File::options().read(true).open(&path)?;
                    let mut cfg: Config = serde_json::from_reader(BufReader::new(file))?;
                    cfg.paths.root = if let Some(parent) = path.parent() {
                        parent.to_path_buf()
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "The path {} points to a config file but doesn't have a parent!",
                                path
                            ),
                        )
                        .into());
                    };
                    cfg
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("The path {} is neither a config file nor a path to a folder containing a config!", path)
                    ).into());
                };

                if log_to_file {
                    config.system.log_to_file = log_to_file;
                }

                let mode = match threads {
                    None => {
                        log::info!("No threads configured, falling back to most optimal mode!");
                        ApplicationMode::Multi(None)
                    }
                    Some(0) => {
                        log::info!("#Threads set to 0, falling back to most optimal mode!");
                        ApplicationMode::Multi(None)
                    }
                    Some(1) => {
                        log::info!("#Threads set to 1, going single mode!");
                        ApplicationMode::Single
                    }
                    Some(threads) => {
                        log::info!("#Threads set to {threads}, going single mode!");
                        ApplicationMode::Multi(Some(unsafe {
                            NonZeroUsize::new_unchecked(threads)
                        }))
                    }
                };

                Ok(Instruction::RunInstruction(RunInstruction {
                    mode,
                    config,
                    seeds: None,
                    recover_mode: true,
                }))
            }
            RunMode::VIEW {
                path,
                internals,
                extracted_links,
                headers,
            } => {
                let path = Utf8PathBuf::from(path);

                let config = if path.is_dir() {
                    let mut cfg: Config = try_load_from_path(&path)?;
                    cfg.paths.root = path;
                    cfg
                } else if path.is_file() {
                    let file = File::options().read(true).open(&path)?;
                    let mut cfg: Config = serde_json::from_reader(BufReader::new(file))?;
                    cfg.paths.root = if let Some(parent) = path.parent() {
                        parent.to_path_buf()
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "The path {} points to a config file but doesn't have a parent!",
                                path
                            ),
                        )
                        .into());
                    };
                    cfg
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("The path {} is neither a config file nor a path to a folder containing a config!", path)
                    ).into());
                };

                println!("{}\n\n{}\n\n\n", ATRA_WELCOME, ATRA_LOGO);

                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Fatal: Was not able to initialize runtime!");
                runtime.block_on(async move {
                    let local = LocalContext::new_without_runtime(config)
                        .expect("Was not able to load context for reading!");
                    view(local, internals, extracted_links, headers, false);
                });
                Ok(Instruction::Nothing)
            }
        }
    } else {
        if args.generate_example_config {
            let cfg = create_example_config();
            let root = cfg.paths.root_path();
            std::fs::create_dir_all(root)?;
            match File::options()
                .create(true)
                .write(true)
                .open(root.join("example_config.json"))
            {
                Ok(file) => match serde_json::to_writer_pretty(BufWriter::new(file), &cfg) {
                    Ok(_) => {}
                    Err(err) => {
                        println!("Failed to create the example file: {err}")
                    }
                },
                Err(err) => {
                    println!("Failed to create the example file: {err}")
                }
            }
            Ok(Instruction::Nothing)
        } else {
            Ok(Instruction::Nothing)
        }
    }
}
