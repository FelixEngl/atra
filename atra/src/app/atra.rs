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

use crate::app::consumer::{GlobalError, GlobalErrorConsumer};
use crate::app::instruction::RunInstruction;
use crate::app::logging::configure_logging;
use crate::contexts::local::{LocalContext, LocalContextInitError};
use crate::contexts::traits::*;
use crate::contexts::worker::{WorkerContext, WorkerContextCreationError};
use crate::contexts::Context;
use crate::crawl::{crawl, ErrorConsumer, ExitState};
use crate::link_state::{LinkStateLike, LinkStateManager, RawLinkState};
use crate::queue::{QueueError, SupportsForcedQueueElement, UrlQueue, UrlQueueElement};
use crate::runtime::{
    AtraRuntime, GracefulShutdownWithGuard, OptionalAtraHandle, RuntimeContext, ShutdownReceiver,
};
use crate::sync::{ContinueOrStop, WorkerBarrier};
use crate::url::{AtraUri, UrlWithDepth};
use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io;
use std::num::NonZeroUsize;
use std::sync::Arc;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::select;
use tokio::task::JoinSet;

#[derive(Debug, Error)]
pub enum AtraRunError {
    #[error(transparent)] ContextInitialisation(#[from] LocalContextInitError),
    #[error(transparent)] WorkerContextInitialisation(#[from] WorkerContextCreationError),
    #[error(transparent)] Crawl(#[from] GlobalError),
    #[error(transparent)] Queue(#[from] QueueError),
}

/// The application
pub struct Atra {
    /// The runtime of atra
    handle: OptionalAtraHandle,

    /// The mode of the application
    mode: ApplicationMode,

    /// The hard shutdown
    shutdown: GracefulShutdownWithGuard,
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
        shutdown: GracefulShutdownWithGuard,
        handle: OptionalAtraHandle,
    ) -> Self {
        Self {
            mode,
            shutdown,
            handle,
        }
    }

    pub fn shutdown(&self) -> &GracefulShutdownWithGuard {
        &self.shutdown
    }

    /// Returns the application, the runtime and the master shutdown token.
    /// Canceling the token immediately stops the application.
    pub fn build_with_runtime(mode: ApplicationMode) -> (Self, AtraRuntime) {
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
                GracefulShutdownWithGuard::new(),
                runtime.handle().as_optional(),
            ),
            runtime,
        )
    }

    #[cfg(test)]
    fn create_contained_with(
        mode: ApplicationMode,
        handle: OptionalAtraHandle,
    ) -> (Self, crate::runtime::GracefulShutdown) {
        let shutdown = GracefulShutdownWithGuard::new();
        let graceful_shutdown = shutdown.get().clone();
        let instance = Self::new(mode, shutdown, handle);
        (instance, graceful_shutdown)
    }

    // fn create_contained(mode: ApplicationMode) -> (Self, AtraRuntime, GracefulShutdownBarrier) {
    //     let (notify, shutdown, barrier) = graceful_shutdown();
    //     let (instance, runtime) = Self::build_with_runtime(mode, notify, shutdown);
    //     (instance, runtime, barrier)
    // }

    /// Start the application
    pub async fn run(&mut self, instruction: RunInstruction) -> Result<(), AtraRunError> {
        configure_logging(&instruction.config);
        let result = self.run_without_logger(instruction).await;
        result
    }

    async fn run_without_logger(
        &mut self,
        RunInstruction {
            config,
            seeds,
            recover_mode,
            ..
        }: RunInstruction,
    ) -> Result<(), AtraRunError> {
        let shutdown_and_handle = RuntimeContext::new(self.shutdown.clone(), self.handle.clone());
        let context = Arc::new(LocalContext::new(config, &shutdown_and_handle)?);
        drop(shutdown_and_handle);

        if let Some(seeds) = seeds {
            seeds.fill_queue(context.url_queue()).await;
        }
        if recover_mode {
            let _guard = self.shutdown.guard();
            let queue = context.url_queue();
            for (k, v) in context
                .get_link_state_manager()
                .iter(IteratorMode::Start)
                .filter_map(|value| value.ok())
            {
                let raw = unsafe { RawLinkState::from_slice_unchecked(v.as_ref()) };
                let uri: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();

                if !raw.kind().is_processed_and_stored() {
                    queue.force_enqueue(UrlQueueElement::new(
                        raw.is_seed().is_yes(),
                        0,
                        false,
                        UrlWithDepth::new(uri, raw.depth()),
                    ))?;
                }
            }
        }
        if self.shutdown.get().child().is_shutdown() {
            log::warn!("Shutdown before doing anything!");
            return Ok(());
        }
        match self.mode {
            ApplicationMode::Single => {
                let start = OffsetDateTime::now_utc();
                let mut recrawl_ct = 0;
                loop {
                    let guard = self.shutdown().guard();
                    let shutdown = self.shutdown.get().child().clone();
                    let barrier = WorkerBarrier::new_with_dependence_to(
                        unsafe { NonZeroUsize::new_unchecked(1) },
                        &shutdown,
                    );
                    let value = match crawl(
                        WorkerContext::create(0, recrawl_ct, context.clone())?,
                        shutdown,
                        Arc::new(barrier),
                        GlobalErrorConsumer::new(),
                    )
                    .await
                    {
                        Ok(value) => value,
                        Err(err) => return Err(err.into()),
                    };
                    drop(guard);

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

                    if self.shutdown.get().is_shutdown() {
                        log::info!("Shutting down.");
                        break;
                    }

                    match value {
                        ExitState::Shutdown => {
                            log::info!("Shutting down.");
                            break;
                        }
                        ExitState::NoMoreElements => {
                            log::info!("No more elements!");
                        }
                    }

                    if self.try_recrawls(context.as_ref()).await {
                        recrawl_ct += 1;
                    } else {
                        break;
                    }
                }

                Ok(())
            }
            ApplicationMode::Multi(worker) => {
                let start = OffsetDateTime::now_utc();
                let mut recrawl_ct = 0;

                loop {
                    let mut set = JoinSet::new();
                    let worker_count = worker.unwrap_or(num_cpus());
                    let barrier = Arc::new(WorkerBarrier::new_with_dependence_to(
                        worker_count,
                        self.shutdown.get().child(),
                    ));
                    for i in 0..worker_count.get() {
                        log::info!("Spawn Worker: {i}");
                        let b = barrier.clone();
                        let shutdown = self.shutdown.clone();
                        let context = WorkerContext::create(i, recrawl_ct, context.clone())?;
                        set.spawn(async move {
                            // This has to be a drop guard to make sure, that we do not fail to wait for a thread.
                            let shutdown = shutdown;
                            let context = context;
                            let barrier = b.clone();
                            let (i, state) = loop {
                                if shutdown.get().is_shutdown() {
                                    break (i, ExitState::Shutdown);
                                }
                                if context.can_poll().await {
                                    match crawl(
                                        context.clone(),
                                        shutdown.get().child().clone(),
                                        barrier.clone(),
                                        GlobalErrorConsumer::new(),
                                    )
                                    .await
                                    {
                                        Ok(s) => {
                                            log::info!("Exit {i} with {s}.");
                                            break (i, s);
                                        }
                                        Err(_) => {
                                            log::error!("Encountered some errors.");
                                        }
                                    }
                                } else {
                                    log::debug!("Wait for all stopping.");
                                    let result = select! {
                                        _ = shutdown.get().child().wait() => {
                                            ContinueOrStop::Cancelled(ExitState::NoMoreElements)
                                        }
                                        value = barrier.wait_for_is_cancelled(
                                            &context,
                                            ExitState::NoMoreElements
                                        ) => {
                                            value
                                        }
                                    };

                                    match result {
                                        ContinueOrStop::Continue(_) => continue,
                                        ContinueOrStop::Cancelled(value) => {
                                            log::info!(
                                                "Stopping worker {} after waiting to stop with {}",
                                                i,
                                                value
                                            );
                                            break (i, value);
                                        }
                                    }
                                }
                            };

                            b.trigger_cancellation();
                            (i, state)
                        });
                    }
                    let mut is_stop = false;
                    while let Some(res) = set.join_next().await {
                        match res {
                            Ok((i, s)) => {
                                log::info!("Stopped worker {i} due to {s}.");
                                is_stop |= matches!(s, ExitState::Shutdown)
                            }
                            Err(err) => {
                                log::error!("Thread join error: {err}");
                                log::error!("Trying to shut down in a safe manner...");
                                self.shutdown.get().shutdown();
                            }
                        }
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

                    if is_stop || self.shutdown.get().is_shutdown() {
                        log::info!("Stopped by shutdown.");
                        break;
                    }

                    log::info!("Start to check if we have some kind of recrawl.");

                    if self.try_recrawls(context.as_ref()).await {
                        recrawl_ct += 1;
                    } else {
                        log::info!("Shutting down, because nothing to recrawl.");
                        break;
                    }
                }
                Ok(())
            }
        }
    }

    /// Returns true if there are more thins to crawl
    async fn try_recrawls<C>(&self, context: &C) -> bool
    where
        C: SupportsUrlQueue + SupportsLinkState,
    {
        log::info!("Start to check if we have some kind of recrawl.");

        if context
            .get_link_state_manager()
            .check_if_there_are_any_recrawlable_links()
            .await
        {
            let queue = context.url_queue();
            context
                .get_link_state_manager()
                .collect_recrawlable_links(|is_seed, url| {
                    queue
                        .force_enqueue(UrlQueueElement::new(is_seed.is_yes(), 0, false, url))
                        .unwrap()
                })
                .await;
            log::info!("Finished refilling queue with data.");
            !queue.is_empty().await
        } else {
            false
        }
    }
}

/// The mode of the application
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ApplicationMode {
    Single,
    /// Contains the number of threads to be used
    Multi(Option<NonZeroUsize>),
}

#[cfg(test)]
mod test {
    use super::{ApplicationMode, Atra};
    use crate::app::constants::ATRA_LOGO;
    use crate::app::instruction::RunInstruction;
    use crate::config::crawl::UserAgent;
    use crate::config::Config as AtraConfig;
    use crate::config::{BudgetSetting, CrawlConfig};
    use crate::contexts::local::LocalContext;
    use crate::contexts::traits::{SupportsLinkState, SupportsUrlQueue};
    use crate::crawl::{SlimCrawlResult, StoredDataHint};
    use crate::link_state::{LinkStateKind, LinkStateLike, RawLinkState};
    use crate::seed::SeedDefinition;
    use crate::url::AtraUri;
    use crate::warc_ext::WarcSkipInstruction;
    use log::LevelFilter;
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log4rs::Config;
    use rocksdb::IteratorMode;
    use std::fs::{read_dir, File};
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use time::ext::NumericalDuration;
    use time::Duration;
    use tokio::task::JoinSet;
    use tokio::time::sleep;

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

    async fn execute_crawl(config: AtraConfig, seeds: Option<SeedDefinition>) {
        let (mut app, shutdown) = Atra::create_contained_with(ApplicationMode::Single, None);

        let barrier_copy = shutdown.clone();
        let a = async move {
            log::info!("============ WAITING! ============");
            sleep(20.seconds().try_into().unwrap()).await;
            let _ = barrier_copy.shutdown();
            log::info!("============ STOP! ============");
            ()
        };

        let b = async move {
            app.run_without_logger(RunInstruction {
                config,
                seeds,
                recover_mode: false,
                mode: ApplicationMode::Single,
            })
            .await
            .expect("no errors");
            ()
        };

        let mut x = JoinSet::new();
        x.spawn(a);
        x.spawn(b);
        x.join_all().await;
        shutdown.wait().await;
    }

    fn show_stats(config: AtraConfig) {
        let local = LocalContext::new_without_runtime(config).expect("Should load!");

        println!("{}", local.url_queue().len_blocking());
        println!("{}", local.crawl_db().len());
        println!("{}", local.get_link_state_manager().len());

        println!("=======");
        for (k, v) in local
            .get_link_state_manager()
            .iter(IteratorMode::Start)
            .filter_map(|value| value.ok())
            .map(|(k, v)| {
                let raw = unsafe { RawLinkState::from_slice_unchecked(v.as_ref()) };
                let uri: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
                (uri, raw.as_link_state().into_owned())
            })
        {
            println!("{k}\n    {v:?}");
            assert_ne!(v.kind(), LinkStateKind::ReservedForCrawl);
            assert_ne!(v.kind(), LinkStateKind::Crawled);
            assert!(matches!(
                v.kind(),
                LinkStateKind::Discovered | LinkStateKind::ProcessedAndStored
            ))
        }
        println!("=======");
        for (k, v) in local
            .crawl_db()
            .iter(IteratorMode::Start)
            .filter_map(|value| value.ok())
            .map(|(k, v)| {
                let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
                let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
                (k, v)
            })
        {
            println!("{k}");
            match v.stored_data_hint {
                StoredDataHint::External(value) => {
                    println!("    External: {} - {}", value.exists(), value);
                }
                StoredDataHint::Warc(value) => match value {
                    WarcSkipInstruction::Single {
                        pointer,
                        kind,
                        header_signature_octet_count,
                    } => {
                        println!(
                            "    Single Warc: {} - {} ({}, {}, {:?})",
                            pointer.path().exists(),
                            pointer.path(),
                            kind,
                            header_signature_octet_count,
                            pointer.pointer()
                        );
                    }
                    WarcSkipInstruction::Multiple {
                        pointers,
                        header_signature_octet_count,
                        is_base64,
                    } => {
                        println!(
                            "    Multiple Warc: ({}, {})",
                            is_base64, header_signature_octet_count
                        );
                        for pointer in pointers {
                            println!(
                                "        {} - {} ({}, {}, {:?})",
                                pointer.path().exists(),
                                pointer.path(),
                                is_base64,
                                header_signature_octet_count,
                                pointer.pointer()
                            );
                        }
                    }
                },
                StoredDataHint::InMemory(value) => {
                    println!("    InMemory: {}", value.len());
                }
                StoredDataHint::None => {
                    println!("    None!")
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn can_restart() {
        init();

        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None,
        };
        config.delay = Some(Duration::milliseconds(1000));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());
        let mut config = AtraConfig::new(
            Default::default(),
            Default::default(),
            Default::default(),
            config,
        );

        config.paths.root = "test/atra_run_0".into();

        if config.paths.root.exists() {
            std::fs::remove_dir_all(&config.paths.root).unwrap();
        }
        std::fs::create_dir_all(&config.paths.root).unwrap();

        execute_crawl(
            config.clone(),
            Some(SeedDefinition::Multi(vec![
                "http://www.antsandelephants.de".to_string(),
                "http://www.aperco.info".to_string(),
                "http://www.applab.de/".to_string(),
                "http://www.carefornetworks.de/".to_string(),
                "https://ticktoo.com/".to_string(),
            ])),
        )
        .await;

        show_stats(config.clone());

        execute_crawl(config.clone(), None).await;

        println!("\n\n========\n\n");

        show_stats(config.clone());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn can_multithread() {
        init();
        let (mut app, shutdown) = Atra::create_contained_with(ApplicationMode::Multi(None), None);

        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None,
        };
        config.delay = Some(Duration::milliseconds(1000));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());

        let config = AtraConfig::new(
            Default::default(),
            Default::default(),
            Default::default(),
            config,
        );

        app.run_without_logger(RunInstruction {
            config,
            seeds: Some(SeedDefinition::Multi(vec![
                "http://www.antsandelephants.de".to_string(),
                "http://www.aperco.info".to_string(),
                "http://www.applab.de/".to_string(),
                "http://www.carefornetworks.de/".to_string(),
                "https://ticktoo.com/".to_string(),
            ])),
            recover_mode: false,
            mode: ApplicationMode::Multi(None),
        })
        .await
        .expect("no errors");

        drop(app);
        shutdown.wait().await;
    }
}

pub trait RunContextProvider: Sync + Send + 'static {
    type Context: Context;
    type Error: From<<Self::Context as SupportsSlimCrawlResults>::Error>
        + From<<Self::Context as SupportsLinkSeeding>::Error>
        + From<<Self::Context as SupportsCrawlResults>::Error>
        + From<<<Self::Context as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>
        + From<<Self::Context as SupportsPolling>::Error>
        + From<<Self::Context as SupportsCrawling>::Error>
        + From<QueueError>
        + From<io::Error>
        + Error;

    type ErrorConsumer: ErrorConsumer<Self::Error>;

    fn create_context(&self, worker_id: usize, retry: usize) -> Self::Context;
    fn create_consumer(&self) -> Self::ErrorConsumer;
}

#[cfg(test)]
mod config_test {
    use crate::app::config::try_load_from_path;
    use crate::seed::read_seeds;

    #[test]
    fn can_load() {
        try_load_from_path("test_crawl/atra").expect("Works");
        let _ = read_seeds("test_crawl/atra/seeds.txt").expect("Was not able to read file");
    }
}
