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


use clap::Parser;
use log::info;
use crate::application::{Atra, ApplicationMode};
use crate::args::{consume_args, AtraArgs, ConsumedArgs};
use crate::core::config::Configs;
use crate::core::seeds::seed_definition::SeedDefinition;
use crate::core::shutdown::graceful_shutdown;

// Shutdown logic from https://github.com/tokio-rs/mini-redis/blob/master/src/server.rs

mod args;
mod core;
mod client;
mod application;
mod features;
mod nom_ext;
mod warc;
mod logging;
pub mod util;
mod config;

pub const ATRA_TEXT: &'static str = r#"
        |
|       |       |
|  |    |    |  |
 \ |  /°°°\  | /
  \| /  A  \ |/
   \ \  T  / /
   /\/  R  \/\
  / /\  A  /\ \
 / /  Oo_oO  \ \
| |   ´` ´`   | |
|               |
"#;

fn main() {
    exec_args(args::AtraArgs::parse())
}

fn exec_args(args: AtraArgs) {
    match consume_args(args) {
        ConsumedArgs::RunConfig(mode, seeds, configs) => {
            exec(mode, seeds, configs);
        }
        ConsumedArgs::Nothing => {}
    }
}

/// Execute the
fn exec(application_mode: ApplicationMode, seed_definition: SeedDefinition, configs: Configs){
    let (notify, shutdown, mut barrier) = graceful_shutdown();
    let (mut atra, runtime) =  Atra::build_with_runtime(
        application_mode,
        notify,
        shutdown
    );
    let signal_handler = tokio::signal::ctrl_c();
    info!("{}", ATRA_TEXT);
    runtime.block_on(
        async move {
            tokio::select! {
                res = atra.run(seed_definition, configs) => {
                    if let Err(err) = res {
                        log::error!("Error: {err}");
                    }
                }
                _ = signal_handler => {
                    log::info!("Shutting down.");
                }
            }
            drop(atra);
            barrier.wait().await;
        }
    );
    info!("Exit application.")
}




#[cfg(test)]
mod test {
    use time::Duration;
    use crate::application::{ApplicationMode};
    use crate::args::{AtraArgs, RunMode};
    use crate::core::config::crawl::UserAgent;
    use crate::core::config::{BudgetSetting, Configs, CrawlConfig};
    use crate::core::seeds::seed_definition::SeedDefinition;
    use crate::{exec, exec_args};

    #[test]
    pub fn can_generate_example_config(){
        let args = AtraArgs {
            mode: None,
            generate_example_config: true
        };
        exec_args(args);
    }

    #[test]
    pub fn can_call_single_crawl(){
        let args = AtraArgs {
            mode: Some(
                RunMode::SINGLE {
                    log_level: log::LevelFilter::Trace,
                    seeds: SeedDefinition::Single("https://choosealicense.com/".to_string()),
                    session_name: Some("test".to_string()),
                    depth: 2,
                    absolute: true,
                    timeout: None,
                    agent: UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string()),
                    log_to_file: true
                }
            ),
            generate_example_config: false
        };

        exec_args(args);
    }

    #[test]
    pub fn can_call_multi_crawl(){
        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None
        };
        config.delay = Some(Duration::milliseconds(300));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());

        exec(
            ApplicationMode::Multi(None),
            SeedDefinition::Multi(vec![
                "http://www.antsandelephants.de".to_string(),
                "http://www.aperco.info".to_string(),
                "http://www.applab.de/".to_string(),
                "http://www.carefornetworks.de/".to_string(),
                "https://ticktoo.com/".to_string(),
            ]),
            Configs::new(
                Default::default(),
                Default::default(),
                config,
                Default::default(),
            )
        )
    }
}