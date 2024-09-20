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

use log::info;

mod args;
mod atra;
mod constants;
pub mod consumer;
mod logging;

#[cfg(test)]
mod terminal;
mod config;
mod instruction;

use atra::{Atra};
pub use args::AtraArgs;
pub use atra::ApplicationMode;
use crate::app::instruction::{prepare_instruction, Instruction, RunInstruction};
use crate::runtime::{GracefulShutdown, ShutdownReceiverWithWait, Shutdown};

pub fn exec_args(args: AtraArgs) {
    match prepare_instruction(args) {
        Ok(Instruction::RunInstruction(instruction)) => {
            execute(instruction);
        }
        Ok(Instruction::Nothing) => {}
        Err(err) => {
            log::error!("Failed with: {err}");
        }
    }
}

/// Execute the
fn execute(instruction: RunInstruction) {
    let shutdown_with_guard = GracefulShutdown::new();
    let shutdown = shutdown_with_guard.create_shutdown();
    let (mut atra, runtime) = Atra::build_with_runtime(instruction.mode, shutdown_with_guard);
    let signal_handler = tokio::signal::ctrl_c();
    runtime.block_on(async move {
        tokio::select! {
            res = atra.run(instruction) => {
                if let Err(err) = res {
                    log::error!("Error: {err}");
                }
            }
            _ = signal_handler => {
                log::info!("Shutting down.");
                shutdown.shutdown();
            }
        }
        drop(atra);
        shutdown.wait().await;
    });
    info!("Exit application.")
}

#[cfg(test)]
mod test {
    use crate::app::args::RunMode;
    use crate::app::atra::ApplicationMode;
    use crate::app::{execute, AtraArgs};
    use crate::config::crawl::UserAgent;
    use crate::config::{BudgetSetting, Config, CrawlConfig};
    use crate::seed::SeedDefinition;
    use time::Duration;
    use crate::app::instruction::RunInstruction;

    #[test]
    pub fn can_generate_example_config() {
        let args = AtraArgs {
            mode: None,
            generate_example_config: true,
        };
        crate::exec_args(args);
    }

    #[test]
    pub fn can_call_single_crawl() {
        let args = AtraArgs {
            mode: Some(RunMode::SINGLE {
                log_level: log::LevelFilter::Trace,
                seeds: SeedDefinition::Single("https://choosealicense.com/".to_string()),
                session_name: Some("test".to_string()),
                depth: 2,
                absolute: true,
                timeout: None,
                agent: UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string()),
                log_to_file: true,
                delay: None,
            }),
            generate_example_config: false,
        };

        crate::exec_args(args);
    }

    #[test]
    pub fn can_call_multi_crawl() {
        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None,
        };
        config.delay = Some(Duration::milliseconds(300));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());

        execute(
            RunInstruction {
                mode: ApplicationMode::Multi(None),
                config: Config::new(
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    config,
                ),
                seeds: Some(
                    SeedDefinition::Multi(vec![
                        "http://www.antsandelephants.de".to_string(),
                        "http://www.aperco.info".to_string(),
                        "http://www.applab.de/".to_string(),
                        "http://www.carefornetworks.de/".to_string(),
                        "https://ticktoo.com/".to_string(),
                    ])
                ),
                recover_mode: false
            }
        )
    }
}
