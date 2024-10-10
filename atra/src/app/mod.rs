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

mod args;
mod atra;
mod constants;
pub mod consumer;
mod logging;

mod config;
mod instruction;
#[cfg(test)]
mod terminal;
mod view;

use crate::app::instruction::{prepare_instruction, Instruction, RunInstruction};
use anyhow::Error;
pub use args::AtraArgs;
pub use atra::ApplicationMode;
use atra::Atra;

/// Execute the [`args`]
pub fn exec_args(args: AtraArgs) {
    match prepare_instruction(args) {
        Ok(Instruction::RunInstruction(instruction)) => {
            execute(instruction);
        }
        Ok(Instruction::Nothing) => {}
        Err(err) => {
            println!("Failed with: {err}");
        }
    }
}

/// Execute the [`instruction`]
fn execute(instruction: RunInstruction) {
    let (mut atra, runtime) = Atra::build_with_runtime(instruction.mode);

    runtime.block_on(async move {
        let shutdown = atra.shutdown().get().clone();

        let shutdown_result = {
            let ctrl_c = tokio::signal::ctrl_c();
            let future = atra.run(instruction);
            tokio::pin!(future);

            let mut shutdown_result: Option<Result<(), Error>> = None;

            tokio::select! {
                res = &mut future => {
                    log::info!("Crawl finished.");
                    shutdown_result.replace(res);
                }
                _ = ctrl_c => {
                    log::info!("Starting with shutdown by CTRL-C.");
                    shutdown.shutdown();
                }
            }

            if let Some(shutdown_result) = shutdown_result {
                shutdown_result
            } else {
                log::info!("Wait for workers to stop...");
                future.await
            }
        };

        if let Err(err) = shutdown_result {
            log::error!("Exit with error: {err}");
        }
        drop(atra);
        log::info!("Waiting for complete shutdown...");
        shutdown.wait().await;
    });
    log::info!("Complete shutdown.")
}

#[cfg(test)]
mod test {
    use crate::app::args::RunMode;
    use crate::app::atra::ApplicationMode;
    use crate::app::instruction::RunInstruction;
    use crate::app::{execute, AtraArgs};
    use crate::config::crawl::UserAgent;
    use crate::config::{Config, CrawlConfig};
    use crate::seed::SeedDefinition;
    use time::Duration;
    use crate::budget::BudgetSetting;

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

        execute(RunInstruction {
            mode: ApplicationMode::Multi(None),
            config: Config::new(
                Default::default(),
                Default::default(),
                Default::default(),
                config,
            ),
            seeds: Some(SeedDefinition::Multi(vec![
                "http://www.antsandelephants.de".to_string(),
                "http://www.aperco.info".to_string(),
                "http://www.applab.de/".to_string(),
                "http://www.carefornetworks.de/".to_string(),
                "https://ticktoo.com/".to_string(),
            ])),
            recover_mode: false,
        })
    }
}
