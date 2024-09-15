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

use crate::app::consumer::GlobalErrorConsumer;
use crate::app::logging::configure_logging;
use crate::config::Configs;
use crate::contexts::local::LocalContext;
use crate::contexts::traits::{SupportsLinkState, SupportsMetaInfo, SupportsUrlQueue};
use crate::contexts::worker::WorkerContext;
use crate::crawl::crawl;
use crate::link_state::LinkStateManager;
use crate::runtime::{
    AtraRuntime, GracefulShutdown, OptionalAtraHandle, RuntimeContext, ShutdownReceiver,
    ShutdownSignalSender,
};
use crate::seed::SeedDefinition;
use crate::sync::barrier::WorkerBarrier;
use cfg_if::cfg_if;
use std::num::NonZeroUsize;
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::task::JoinSet;

cfg_if! {
    if #[cfg(test)] {
        use crate::runtime::{graceful_shutdown, GracefulShutdownBarrier};
    }
}

/// The application
pub struct Atra {
    /// The runtime of atra
    handle: OptionalAtraHandle,

    /// The mode of the application
    mode: ApplicationMode,

    /// Broadcasts a shutdown signal to all active connections.
    ///
    /// The initial `shutdown` trigger is provided by the `run` caller. The
    /// server is responsible for gracefully shutting down active connections.
    /// When a connection task is spawned, it is passed a broadcast receiver
    /// handle. When a graceful shutdown is initiated, a `()` value is sent via
    /// the broadcast::Sender. Each active connection receives it, reaches a
    /// safe terminal state, and completes the task.
    _notify_shutdown: ShutdownSignalSender,

    /// Used as part of the graceful shutdown process to wait for client
    /// connections to complete processing.
    ///
    /// Tokio channels are closed once all `Sender` handles go out of scope.
    /// When a channel is closed, the receiver receives `None`. This is
    /// leveraged to detect all connection handlers completing. When a
    /// connection handler is initialized, it is assigned a clone of
    /// `shutdown_complete_tx`. When the listener shuts down, it drops the
    /// sender held by this `shutdown_complete_tx` field. Once all handler tasks
    /// complete, all clones of the `Sender` are also dropped. This results in
    /// `shutdown_complete_rx.recv()` completing with `None`. At this point, it
    /// is safe to exit the server process.
    shutdown: GracefulShutdown,
}

/// From tokio
fn num_cpus() -> NonZeroUsize {
    const ENV_WORKER_THREADS: &str = "TOKIO_WORKER_THREADS";

    match std::env::var(ENV_WORKER_THREADS) {
        Ok(s) => {
            let n = s.parse().unwrap_or_else(|e| {
                panic!(
                    "\"{}\" must be usize, error: {}, value: {}",
                    ENV_WORKER_THREADS, e, s
                )
            });
            assert!(n > 0, "\"{}\" cannot be set to 0", ENV_WORKER_THREADS);
            unsafe { NonZeroUsize::new_unchecked(n) }
        }
        Err(std::env::VarError::NotPresent) => NonZeroUsize::new(usize::max(1, num_cpus::get()))
            .unwrap_or(unsafe { NonZeroUsize::new_unchecked(1) }),
        Err(std::env::VarError::NotUnicode(e)) => {
            panic!(
                "\"{}\" must be valid unicode, error: {:?}",
                ENV_WORKER_THREADS, e
            )
        }
    }
}

impl Atra {
    pub fn new(
        mode: ApplicationMode,
        notify_shutdown: ShutdownSignalSender,
        shutdown: GracefulShutdown,
        handle: OptionalAtraHandle,
    ) -> Self {
        Self {
            mode,
            _notify_shutdown: notify_shutdown,
            shutdown,
            handle,
        }
    }

    pub fn build_with_runtime(
        mode: ApplicationMode,
        notify_shutdown: ShutdownSignalSender,
        shutdown: GracefulShutdown,
    ) -> (Self, AtraRuntime) {
        let runtime = match &mode {
            ApplicationMode::Single => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Fatal: Was not able to initialize runtime!"),
            ApplicationMode::Multi(threads) => {
                if let Some(t) = threads {
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .worker_threads(t.get())
                        .build()
                        .expect("Fatal: Was not able to initialize runtime!")
                } else {
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("Fatal: Was not able to initialize runtime!")
                }
            }
        };

        let runtime = AtraRuntime::new(runtime, None);

        (
            Self::new(
                mode,
                notify_shutdown,
                shutdown,
                runtime.handle().as_optional(),
            ),
            runtime,
        )
    }

    #[cfg(test)]
    fn create_contained_with(
        mode: ApplicationMode,
        handle: OptionalAtraHandle,
    ) -> (Self, GracefulShutdownBarrier) {
        let (notify, shutdown, barrier) = graceful_shutdown();
        let instance = Self::new(mode, notify, shutdown, handle);
        (instance, barrier)
    }

    // fn create_contained(mode: ApplicationMode) -> (Self, AtraRuntime, GracefulShutdownBarrier) {
    //     let (notify, shutdown, barrier) = graceful_shutdown();
    //     let (instance, runtime) = Self::build_with_runtime(mode, notify, shutdown);
    //     (instance, runtime, barrier)
    // }

    /// Start the application
    pub async fn run(
        &mut self,
        seeds: SeedDefinition,
        configs: Configs,
    ) -> Result<(), anyhow::Error> {
        configure_logging(&configs);
        self.run_without_logger(seeds, configs).await
    }

    async fn run_without_logger(
        &self,
        seeds: SeedDefinition,
        configs: Configs,
    ) -> Result<(), anyhow::Error> {
        match self.mode {
            ApplicationMode::Single => {
                let start = OffsetDateTime::now_utc();

                let shutdown_and_handle = RuntimeContext::new(
                    self.shutdown.new_guard_instance().to_unsafe(),
                    self.handle.clone(),
                );

                let context = Arc::new(
                    LocalContext::new(configs, shutdown_and_handle)
                        .await
                        .unwrap(),
                );
                let barrier = WorkerBarrier::new(unsafe { NonZeroUsize::new_unchecked(1) });
                seeds.fill_queue(context.url_queue()).await;
                crawl(
                    WorkerContext::create(0, context.clone()).await?,
                    self.shutdown.weak_handle(),
                    Arc::new(barrier),
                    GlobalErrorConsumer::new(),
                )
                .await
                .expect("Failed the crawl.");
                let time_needed = OffsetDateTime::now_utc() - start;
                log::info!(
                    "Needed {} for discovering {} websites",
                    time_needed,
                    context.discovered_websites()
                );
                log::info!(
                    "Needed {} for crawling {} websites",
                    time_needed,
                    context
                        .get_link_state_manager()
                        .crawled_websites()
                        .map(|value| value.to_string())
                        .unwrap_or("# ERROR COUNTING#".to_string())
                );
                return Ok(());
            }
            ApplicationMode::Multi(worker) => {
                let start = OffsetDateTime::now_utc();
                let shutdown_and_handle = RuntimeContext::new(
                    self.shutdown.new_guard_instance().to_unsafe(),
                    self.handle.clone(),
                );

                let context = Arc::new(
                    LocalContext::new(configs, shutdown_and_handle)
                        .await
                        .unwrap(),
                );
                seeds.fill_queue(context.url_queue()).await;
                let mut set = JoinSet::new();
                let worker_count = worker.unwrap_or(num_cpus());
                let barrier = Arc::new(WorkerBarrier::new(worker_count));
                for i in 0..worker_count.get() {
                    log::info!("Spawn Worker: {i}");
                    let b = barrier.clone();
                    let s = self.shutdown.clone();
                    let context = WorkerContext::create(i, context.clone()).await?;
                    set.spawn(async move {
                        let context = context;
                        while context.can_poll().await {
                            match crawl(
                                context.clone(),
                                s.clone(),
                                b.clone(),
                                GlobalErrorConsumer::new(),
                            )
                            .await
                            {
                                Ok(s) => {
                                    log::info!("Exit {i} with {s}.");
                                    break;
                                }
                                Err(_) => {
                                    log::error!("Encountered some errors.");
                                }
                            }
                        }

                        b.trigger_cancellation();
                        i
                    });
                }
                while let Some(res) = set.join_next().await {
                    log::info!("Stopped worker {res:?}.")
                }
                let time_needed = OffsetDateTime::now_utc() - start;
                log::info!(
                    "Needed {} for discovering {} websites",
                    time_needed,
                    context.discovered_websites()
                );
                log::info!(
                    "Needed {} for crawling {} websites",
                    time_needed,
                    context
                        .get_link_state_manager()
                        .crawled_websites()
                        .map(|value| value.to_string())
                        .unwrap_or("# ERROR COUNTING#".to_string())
                );
                Ok(())
            }
        }
    }
}

/// The mode of the application
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ApplicationMode {
    Single,
    /// Contains the number of threads to be used
    Multi(Option<NonZeroUsize>),
}

#[cfg(test)]
mod test {
    use super::{ApplicationMode, Atra};
    use crate::app::constants::ATRA_LOGO;
    use crate::config::crawl::UserAgent;
    use crate::config::{BudgetSetting, Configs, CrawlConfig};
    use crate::runtime::OptionalAtraHandle;
    use crate::seed::SeedDefinition;
    use log::LevelFilter;
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log4rs::Config;
    use std::fs::{read_dir, File};
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use time::Duration;

    fn recurse(path: impl AsRef<Path>) -> Vec<PathBuf> {
        let Ok(entries) = read_dir(path) else {
            return vec![];
        };
        entries
            .flatten()
            .flat_map(|entry| {
                let Ok(meta) = entry.metadata() else {
                    return vec![];
                };
                if meta.is_dir() {
                    return recurse(entry.path());
                }
                if meta.is_file() {
                    return vec![entry.path()];
                }
                vec![]
            })
            .collect()
    }

    #[test]
    fn check() {
        let mut s = String::new();
        for path in recurse("C:\\git\\atra\\atra\\src") {
            File::open(&path).unwrap().read_to_string(&mut s).unwrap();
            if !s.starts_with("//Copyright") {
                println!("{}", path.to_str().unwrap());
            }
            s.clear();
        }

        println!("{}", ATRA_LOGO)
    }

    fn init() {
        // let stdout = ConsoleAppender::builder().build();

        let requests = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l};{I} - {d} - {m}{n}")))
            .build("log/out.log")
            .unwrap();

        let config = Config::builder()
            // .appender(Appender::builder().build("stdout", Box::new(stdout)))
            .appender(Appender::builder().build("out", Box::new(requests)))
            .logger(Logger::builder().build("atra", LevelFilter::Trace))
            .build(Root::builder().appender("out").build(LevelFilter::Warn))
            .unwrap();

        let _ = log4rs::init_config(config).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn can_multithread() {
        init();
        let (app, mut barrier) =
            Atra::create_contained_with(ApplicationMode::Multi(None), OptionalAtraHandle::None);

        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None,
        };
        config.delay = Some(Duration::milliseconds(1000));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());

        let configs = Configs::new(
            Default::default(),
            Default::default(),
            Default::default(),
            config,
        );

        app.run_without_logger(
            SeedDefinition::Multi(vec![
                "http://www.antsandelephants.de".to_string(),
                "http://www.aperco.info".to_string(),
                "http://www.applab.de/".to_string(),
                "http://www.carefornetworks.de/".to_string(),
                "https://ticktoo.com/".to_string(),
            ]),
            configs,
        )
        .await
        .expect("no errors");

        drop(app);
        barrier.wait().await;
    }
}

#[cfg(test)]
mod config_test {
    use crate::config::Configs;
    use crate::seed::read_seeds;

    #[test]
    fn can_load() {
        Configs::load_from("test_crawl/atra").expect("Works");
        let _ = read_seeds("test_crawl/atra/seeds.txt").expect("Was not able to read file");
    }
}
