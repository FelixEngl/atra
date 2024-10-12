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

use crate::config::crawl::UserAgent;
use crate::seed::SeedDefinition;
use clap::{Parser, Subcommand};
use std::str::FromStr;

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
        /// The delay in milliseconds.
        #[arg(short = 'w', long)]
        delay: Option<u64>,
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
        /// Overrides the sub-root dir name from atra_xxx_xxx.
        /// If it is an absolute path the complete root is replaced.
        #[arg(long)]
        override_root_dir_name: Option<String>,
        /// Log to file
        #[arg(long)]
        log_to_file: bool,
        /// Seed to be crawled
        seeds: SeedDefinition,
    },
    /// Continue a crawl that was somehow ended.
    RECOVER {
        /// The number of threads used by this application.
        #[arg(short, long)]
        threads: Option<usize>,
        /// Log to file
        #[arg(long)]
        log_to_file: bool,
        /// The path to the folder with the atra data
        path: String,
    },
    /// Initializes Atra for Multi by creating the default config filee
    INIT,

    /// View the content of the crawl
    VIEW {
        /// Show internal states of atra
        #[arg(short, long)]
        internals: bool,
        /// Show the extracted link of every page
        #[arg(short, long)]
        extracted_links: bool,
        /// Show the headers of every page
        #[arg(short, long)]
        headers: bool,
        /// The path to the folder with the atra data
        path: String,
    },
    /// Dump the warc file paths and the url metadata to a folder.
    DUMP {
        /// Directory for the dumps
        #[arg(short, long)]
        output_dir: Option<String>,
        /// The path to the crawl
        crawl_path: String,
    }
}

#[cfg(test)]
mod test {
    use crate::app::instruction::{prepare_instruction, Instruction};
    use crate::app::{execute, AtraArgs};
    use crate::config::crawl::UserAgent;
    use crate::seed::SeedDefinition;
    use log::max_level;

    #[test]
    fn works() {
        let args = AtraArgs {
            generate_example_config: false,
            mode: Some(crate::app::args::RunMode::SINGLE {
                session_name: None,
                depth: 1,
                log_to_file: true,
                delay: None,
                absolute: false,
                agent: UserAgent::Default,
                seeds: SeedDefinition::Single(
                    "https://www.arche-naturkueche.de/de/rezepte/uebersicht.php".to_string(),
                ),
                log_level: max_level(),
                timeout: None,
            }),
        };

        let args = prepare_instruction(args);

        match args {
            Ok(Instruction::RunInstruction(instruction)) => {
                execute(instruction).expect("This should work!");
            }
            Ok(Instruction::Nothing) => {}
            _ => {}
        }
    }
}
