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

mod intervals;
pub(super) mod result;
mod sitemaps;
pub(super) mod slim;

#[cfg(test)]
#[allow(unused_imports)]
pub use result::test::*;

#[cfg(test)]
#[allow(unused_imports)]
pub use crate::blacklist::ManagedBlacklist;
use crate::blacklist::{Blacklist, BlacklistManager};
use crate::client::traits::AtraClient;
use crate::config::BudgetSetting;
use crate::contexts::traits::{
    SupportsBlackList, SupportsConfigs, SupportsCrawlResults, SupportsCrawling,
    SupportsDomainHandling, SupportsFileSystemAccess, SupportsGdbrRegistry, SupportsLinkSeeding,
    SupportsLinkState, SupportsRobotsManager, SupportsSlimCrawlResults, SupportsUrlQueue,
};
use crate::crawl::crawler::intervals::InvervalManager;
use crate::crawl::crawler::result::CrawlResult;
use crate::crawl::crawler::sitemaps::retrieve_and_parse;
use crate::crawl::ErrorConsumer;
use crate::data::{process, RawData, RawVecData};
use crate::fetching::ResponseData;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::{AtraFileInformation, FileFormatData};
use crate::io::fs::AtraFS;
use crate::link_state::{
    IsSeedYesNo, LinkStateKind, LinkStateLike, LinkStateManager, RecrawlYesNo,
};
use crate::queue::{QueueError, UrlQueue, UrlQueueElement};
use crate::recrawl_management::DomainLastCrawledManager;
use crate::robots::{GeneralRobotsInformation, RobotsInformation};
use crate::runtime::ShutdownReceiver;
use crate::seed::BasicSeed;
use crate::toolkit::detect_language;
use crate::url::UrlWithDepth;
use itertools::Itertools;
use log::LevelFilter;
use sitemap::structs::Location;
use smallvec::SmallVec;
use std::collections::{HashSet, VecDeque};
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::Write;
use std::sync::Arc;
use strum::EnumString;
use time::OffsetDateTime;

/// A crawler for a single website. Starts from the provided `seed` and
#[derive(Debug)]
pub struct CrawlTask<S, Client> {
    /// The seed of the crawl task
    seed: S,

    /// The request client. Stored for re-use between runs.
    client: Client,

    /// All URLs visited.
    links_visited: HashSet<UrlWithDepth>,
}

impl<S, Client> CrawlTask<S, Client> {
    /// Creates a new instance of a WebsiteCrawler
    pub fn new(seed: S, client: Client) -> Self {
        Self {
            seed,
            client,
            links_visited: Default::default(),
        }
    }
}

impl<S, Client> CrawlTask<S, Client>
where
    S: BasicSeed,
    Client: AtraClient,
{
    #[inline(always)]
    async fn update_linkstate_no_meta<C, E, EC>(
        handler: &EC,
        context: &C,
        target: &UrlWithDepth,
        link_state_type: LinkStateKind,
    ) -> Result<(), EC::Error>
    where
        C: SupportsLinkState,
        E: From<<<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>,
        EC: ErrorConsumer<E>,
    {
        Self::update_linkstate(handler, context, target, link_state_type, None, None).await
    }

    async fn update_linkstate<C, E, EC>(
        handler: &EC,
        context: &C,
        target: &UrlWithDepth,
        link_state_type: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
    ) -> Result<(), EC::Error>
    where
        C: SupportsLinkState,
        E: From<<<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>,
        EC: ErrorConsumer<E>,
    {
        log::trace!("Update {link_state_type}: ``{}``", target);
        match context
            .get_link_state_manager()
            .update_link_state_no_payload(target, link_state_type, is_seed, recrawl)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => handler.consume_crawl_error(error.into()),
        }
    }

    async fn pack_shutdown<C, E, EC>(
        handler: &EC,
        context: &C,
        target: &UrlWithDepth,
        link_state_type: LinkStateKind,
    ) -> Result<(), EC::Error>
    where
        C: SupportsLinkState,
        E: From<<<C as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>,
        EC: ErrorConsumer<E>,
    {
        if Self::update_linkstate(handler, context, target, link_state_type, None, None)
            .await
            .is_err()
        {
            log::info!("Continue shutdown without escalating the error.");
        }
        Ok(())
    }

    /// The crawl method.
    pub async fn run<Cont, Shutdown, E, EC>(
        &mut self,
        context: &Cont,
        shutdown: Shutdown,
        consumer: &EC,
    ) -> Result<(), EC::Error>
    where
        Cont: SupportsGdbrRegistry
            + SupportsConfigs
            + SupportsRobotsManager
            + SupportsBlackList
            + SupportsLinkState
            + SupportsSlimCrawlResults
            + SupportsFileSystemAccess
            + SupportsCrawlResults
            + SupportsLinkSeeding
            + SupportsUrlQueue
            + SupportsCrawling
            + SupportsDomainHandling,
        Shutdown: ShutdownReceiver,
        E: From<<Cont as SupportsSlimCrawlResults>::Error>
            + From<<Cont as SupportsLinkSeeding>::Error>
            + From<<Cont as SupportsCrawlResults>::Error>
            + From<<<Cont as SupportsLinkState>::LinkStateManager as LinkStateManager>::Error>
            + From<<Cont as SupportsCrawling>::Error>
            + From<QueueError>
            + From<io::Error>
            + Display,
        EC: ErrorConsumer<E>,
    {
        let configuration = &context.configs().crawl;

        if shutdown.is_shutdown() {
            return Ok(());
        }

        let configured_robots = Arc::new(
            GeneralRobotsInformation::new(
                context.get_robots_manager(),
                self.client.user_agent().to_string(),
                configuration.max_robots_age.clone(),
            )
            .bind_to_domain(&self.client, self.seed.url())
            .await,
        );

        let budget = configuration
            .budget
            .get_budget_for(&self.seed.origin())
            .clone();

        log::info!("Seed: {}, {}", self.seed.url(), budget);

        let blacklist = context.get_blacklist_manager().get_blacklist().await;

        log::debug!("Local blacklist initialized {:}", self.seed.url());
        let mut queue = VecDeque::with_capacity(128);

        queue.push_back((true, self.seed.url().clone()));

        match context.register_seed(&self.seed).await {
            Ok(_) => {}
            Err(err) => {
                consumer.consume_crawl_error(err.into())?;
            }
        }

        let checker = UrlChecker {
            configured_robots: configured_robots.as_ref(),
            blacklist: &blacklist,
            budget: &budget,
        };

        // todo: do not ignore sitemaps?

        let mut interval_manager =
            InvervalManager::new(&self.client, &configuration, configured_robots.clone());

        if !context.configs().crawl.ignore_sitemap {
            for value in retrieve_and_parse(
                &self.client,
                &self.seed.url(),
                configured_robots.as_ref(),
                &mut interval_manager,
                None,
            )
            .await
            .urls
            {
                match value.loc {
                    Location::None => {}
                    Location::Url(url) => match UrlWithDepth::with_base(self.seed.url(), url) {
                        Ok(url) => {
                            queue.push_back((false, url));
                        }
                        Err(err) => {
                            log::debug!("Failed to parse url from sitemap: {err}");
                        }
                    },
                    Location::ParseErr(err) => {
                        log::info!("Had error parsing sitemap: {err}");
                    }
                }
            }
        }
        let origin = self.seed.origin();
        let manager = context.get_domain_manager();

        if let Some(recrawl_interval) = configuration
            .budget
            .get_budget_for(origin)
            .get_recrawl_interval()
        {
            let needs_recrawl_protection = if let Ok(Some(value)) = context
                .get_link_state_manager()
                .get_link_state(self.seed.url())
                .await
            {
                value.kind().is_processed_and_stored()
            } else {
                false
            };

            if needs_recrawl_protection {
                if let Some(time) = manager.get_last_access(origin).await {
                    let time_since_last_access = OffsetDateTime::now_utc() - time;
                    if time_since_last_access.le(recrawl_interval) {
                        log::debug!("The domain is on cooldown. Last Access: {time_since_last_access}, Recrawl Interval: {recrawl_interval}");
                        return match context
                            .url_queue()
                            .enqueue(UrlQueueElement::new(
                                self.seed.is_original_seed(),
                                0,
                                false,
                                self.seed.url().clone(),
                            ))
                            .await
                        {
                            Ok(_) => Ok(()),
                            Err(err) => consumer.consume_crawl_error(err.into()),
                        };
                    }
                }
            }
        }

        while let Some((is_seed, target)) = queue.pop_front() {
            let old_link_state = match context
                .get_link_state_manager()
                .get_link_state(self.seed.url())
                .await
            {
                Ok(value) => value.map(|value| value.kind()),
                Err(err) => return consumer.consume_crawl_error(err.into()),
            };

            if shutdown.is_shutdown() {
                let _ = Self::update_linkstate_no_meta(
                    consumer,
                    context,
                    &target,
                    old_link_state.unwrap_or(LinkStateKind::Discovered),
                )
                .await;
                return Ok(());
            }
            log::trace!("Queue.len() => {}", queue.len());

            if !checker.check_if_allowed(self, &target).await {
                log::debug!("Dropped Seed: {}", target);
                let _ = Self::update_linkstate_no_meta(
                    consumer,
                    context,
                    &target,
                    old_link_state.unwrap_or(LinkStateKind::Discovered),
                )
                .await;
                continue;
            }

            manager.register_access(origin).await;
            match context.retrieve_slim_crawled_website(&target).await {
                Ok(value) => {
                    if let Some(already_crawled) = value {
                        if let Some(recrawl) = configuration
                            .budget
                            .get_budget_for(origin)
                            .get_recrawl_interval()
                        {
                            let time_since_crawled =
                                OffsetDateTime::now_utc() - already_crawled.meta.created_at;

                            if time_since_crawled.ge(recrawl) {
                                log::debug!("The url was already crawled.");
                                continue;
                            }
                            match Self::update_linkstate_no_meta(
                                consumer,
                                context,
                                &target,
                                LinkStateKind::ReservedForCrawl,
                            )
                            .await
                            {
                                Ok(_) => {}
                                Err(_) => {
                                    let _ = Self::update_linkstate_no_meta(
                                        consumer,
                                        context,
                                        &target,
                                        old_link_state.unwrap_or(LinkStateKind::Discovered),
                                    )
                                    .await;
                                    log::info!("Failed setting of linkstate of {target}, continue without further processing.");
                                    continue;
                                }
                            }
                        } else {
                            log::debug!("The url {} was already crawled.", target);
                            continue;
                        }
                    } else {
                        match Self::update_linkstate(
                            consumer,
                            context,
                            &target,
                            LinkStateKind::ReservedForCrawl,
                            Some(is_seed.into()),
                            Some(checker.has_recrawl().into()),
                        )
                        .await
                        {
                            Ok(_) => {}
                            Err(_) => {
                                let _ = Self::update_linkstate_no_meta(
                                    consumer,
                                    context,
                                    &target,
                                    old_link_state.unwrap_or(LinkStateKind::Discovered),
                                )
                                .await;
                                log::info!("Failed setting of linkstate of {target}, continue without further processing.");
                                continue;
                            }
                        }
                    }
                }
                Err(err) => {
                    log::warn!(
                        "Failed to get the head information for {target}, try to continue. ${err}"
                    )
                }
            }

            if shutdown.is_shutdown() {
                return Self::pack_shutdown(consumer, context, &target, LinkStateKind::Discovered)
                    .await;
            }
            if log::max_level() == LevelFilter::Trace {
                log::trace!("Interval Start: {} {}", OffsetDateTime::now_utc(), target);
            }
            interval_manager.wait(&target).await;
            if log::max_level() == LevelFilter::Trace {
                log::trace!("Interval End: {}", OffsetDateTime::now_utc());
            }
            log::info!("Crawl: {}", target);
            let url_str = target.try_as_str().into_owned();
            match self.client.retrieve(context, &url_str).await {
                Ok(page) => {
                    if Self::update_linkstate_no_meta(
                        consumer,
                        context,
                        &target,
                        LinkStateKind::Crawled,
                    )
                    .await
                    .is_err()
                    {
                        log::info!("Failed to set link state of {target}.");
                    }

                    log::trace!("Fetched: {}", target);
                    let mut response_data = ResponseData::from_response(page, target.clone());

                    let file_information = AtraFileInformation::determine(
                        context,
                        FileFormatData::from_response(&response_data)
                    );

                    let (language, analyzed, links) =
                        match process(context, &response_data, &file_information).await {
                            Ok(decoded) => {
                                let lang = detect_language(
                                    context,
                                    &response_data,
                                    &file_information,
                                    &decoded,
                                )
                                .ok()
                                .flatten();

                                let result = context
                                    .configs()
                                    .crawl
                                    .link_extractors
                                    .extract(
                                        context,
                                        &response_data,
                                        &file_information,
                                        &decoded,
                                        lang.as_ref(),
                                    )
                                    .await;

                                (lang, decoded, result)
                            }
                            Err(err) => {
                                log::error!(
                                    "Failed to extract links for {} with {err}",
                                    &response_data.url
                                );
                                let _ = Self::update_linkstate_no_meta(
                                    consumer,
                                    context,
                                    &target,
                                    LinkStateKind::InternalError,
                                )
                                .await;
                                continue;
                            }
                        };
                    log::trace!("Finished analysis: {}", target);

                    if context.configs().crawl.store_only_html_in_warc {
                        if file_information.format != InterpretedProcessibleFileFormat::HTML {
                            response_data.content = match response_data.content {
                                RawVecData::InMemory { data } => {
                                    let path =
                                        context.fs().create_unique_path_for_dat_file(&url_str);
                                    match File::options().create_new(true).write(true).open(&path) {
                                        Ok(mut out) => match out.write_all(&data) {
                                            Ok(_) => RawData::from_external(path),
                                            Err(err) => {
                                                log::error!("Failed to store {} as file {} with {err}. Keep in memory.", url_str, path);
                                                RawVecData::InMemory { data }
                                            }
                                        },
                                        Err(err) => {
                                            log::error!("Failed to store {} as file {} with {err}. Keep in memory.", url_str, path);
                                            RawVecData::InMemory { data }
                                        }
                                    }
                                }
                                keep => keep,
                            }
                        }
                    }

                    if shutdown.is_shutdown() {
                        return Self::pack_shutdown(
                            consumer,
                            context,
                            &target,
                            LinkStateKind::Discovered,
                        )
                        .await;
                    }
                    log::debug!(
                        "Number of links in {}: {}",
                        response_data.url,
                        links.links.len()
                    );
                    let links = links.to_optional_links();
                    log::trace!("Converted links");
                    if let Some(links) = &links {
                        log::trace!("Handle extracted links");
                        match context.handle_links(&target, links).await {
                            Ok(value) => {
                                log::debug!(
                                    "{}: on_seed links: {}",
                                    response_data.url,
                                    value.len()
                                );
                                for in_seed in value {
                                    if checker.check_if_allowed(self, &in_seed).await {
                                        log::trace!("Queue: {}", target);
                                        queue.push_back((false, in_seed));
                                    } else {
                                        log::debug!("Dropped: {in_seed}");
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to handle links with {err}. Stopping crawl.");
                                let _ = consumer.consume_crawl_error(err.into());
                                return Self::pack_shutdown(
                                    consumer,
                                    context,
                                    &target,
                                    LinkStateKind::Discovered,
                                )
                                .await;
                            }
                        }
                    } else {
                        log::trace!("No links");
                    }
                    self.links_visited.insert(response_data.url.clone());
                    let recognized_encoding = analyzed.encoding();
                    drop(analyzed);
                    if shutdown.is_shutdown() {
                        return Self::pack_shutdown(
                            consumer,
                            context,
                            &target,
                            LinkStateKind::Discovered,
                        )
                        .await;
                    }

                    log::trace!("CrawlResult {}", response_data.url);
                    let result = CrawlResult::new(
                        OffsetDateTime::now_utc(),
                        response_data,
                        links,
                        recognized_encoding,
                        file_information,
                        language,
                    );
                    log::debug!("Store {}", result.meta.url);
                    match context.store_crawled_website(&result).await {
                        Err(err) => {
                            log::error!("Failed to store data for {target}. Stopping crawl. {err}");
                            let _ = consumer.consume_crawl_error(err.into());
                            return Self::pack_shutdown(
                                consumer,
                                context,
                                &target,
                                LinkStateKind::Discovered,
                            )
                            .await;
                        }
                        _ => {
                            log::debug!("Stored: {}", result.meta.url);
                        }
                    }

                    if Self::update_linkstate_no_meta(
                        consumer,
                        context,
                        &target,
                        LinkStateKind::ProcessedAndStored,
                    )
                    .await
                    .is_err()
                    {
                        log::error!("Failed setting of linkstate of {target}.");
                    }
                }
                Err(err) => {
                    log::warn!("Failed to fetch {} with error {}", target, err);

                    if Self::update_linkstate_no_meta(
                        consumer,
                        context,
                        &target,
                        LinkStateKind::InternalError,
                    )
                    .await
                    .is_err()
                    {
                        log::error!("Failed recovery of linkstate of {target}.");
                    }
                }
            }
        }
        Ok(())
    }
}

// Helper structs

/// Internal helper for representing cause for not allowed
#[derive(Debug, EnumString, strum::Display)]
enum NotAllowedReasoning {
    IsAlreadyVisited,
    BlacklistHasMatch,
    RobotSaysNo,
    IsNotInBudget,
}

struct UrlChecker<'a, R: RobotsInformation, B: Blacklist> {
    budget: &'a BudgetSetting,
    configured_robots: &'a R,
    blacklist: &'a B,
}

impl<'a, R: RobotsInformation, B: Blacklist> UrlChecker<'a, R, B> {
    /// return `true` if link:
    ///
    /// - is not already crawled
    /// - is not over crawl budget
    /// - is not blacklisted
    /// - is not forbidden in robot.txt file (if parameter is defined)
    async fn check_if_allowed<T, Client>(
        &self,
        task: &CrawlTask<T, Client>,
        url: &UrlWithDepth,
    ) -> bool
    where
        T: BasicSeed,
        Client: AtraClient,
    {
        let result = !task.links_visited.contains(url)
            && !self.blacklist.has_match_for(&url.try_as_str())
            && self
                .configured_robots
                .check_if_allowed(&task.client, url)
                .await
            && self.budget.is_in_budget(url);

        if result {
            log::trace!("Allowed: {}", url);
            return result;
        }

        match log::max_level() {
            LevelFilter::Trace => {
                let reason = {
                    let mut reasons = SmallVec::<[NotAllowedReasoning; 4]>::new();
                    if task.links_visited.contains(url) {
                        reasons.push(NotAllowedReasoning::IsAlreadyVisited);
                    }
                    if self.blacklist.has_match_for(&url.try_as_str()) {
                        reasons.push(NotAllowedReasoning::BlacklistHasMatch);
                    }
                    if !self
                        .configured_robots
                        .check_if_allowed(&task.client, &url)
                        .await
                    {
                        reasons.push(NotAllowedReasoning::RobotSaysNo);
                    }
                    if !self.budget.is_in_budget(url) {
                        reasons.push(NotAllowedReasoning::IsNotInBudget);
                    }
                    reasons.iter().map(|value| value.to_string()).join(", ")
                };

                log::trace!("Drop-Reasons: {}; Reasons: {}", url, reason);
            }
            _ => {}
        }

        return result;
    }

    pub fn has_recrawl(&self) -> bool {
        self.budget.get_recrawl_interval().is_some()
    }
}

#[cfg(test)]
mod test {
    use crate::config::{BudgetSetting, Config as AtraConfig, CrawlConfig};
    use crate::contexts::traits::{SupportsCrawling, SupportsUrlQueue};
    use crate::crawl::CrawlResult;
    use crate::data::RawData;
    use crate::fetching::FetchedRequestData;
    use crate::queue::UrlQueue;
    use crate::runtime::ShutdownPhantom;
    use crate::seed::UnguardedSeed;
    use crate::test_impls::{FakeClientProvider, FakeResponse, TestContext, TestErrorConsumer};
    use crate::toolkit::header_map_extensions::optional_header_map;
    use crate::toolkit::serde_ext::status_code;
    use crate::url::AtraOriginProvider;
    use log::LevelFilter;
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Config, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use reqwest::header::HeaderMap;
    use reqwest::StatusCode;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;
    use time::Duration;

    fn init() {
        // let stdout = ConsoleAppender::builder().build();

        let requests = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l} - {d} - {m}{n}")))
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

    // #[tokio::test]
    // async fn can_use_default_client() {
    //     let seed = "https://choosealicense.com/"
    //         .parse::<UrlWithDepth>()
    //         .unwrap()
    //         .try_into()
    //         .unwrap();
    //     let crawl_task: WebsiteCrawler<UnguardedSeed> =
    //         WebsiteCrawler::new(&CrawlConfig::default())
    //             .build(seed)
    //             .await;
    //     let in_memory = InMemoryRobotsManager::new();
    //     println!("{:?}", crawl_task.client);
    //     let retrieved = in_memory
    //         .get_or_retrieve(
    //             &crawl_task.client,
    //             &crawl_task.user_agent,
    //             &"https://choosealicense.com/"
    //                 .parse::<UrlWithDepth>()
    //                 .unwrap(),
    //             None,
    //         )
    //         .await;
    //     println!("{:?}", retrieved)
    // }

    fn check_serialisation_value<T: Serialize + DeserializeOwned + Debug + PartialEq + Eq>(
        value: &T,
    ) {
        let serialized = bincode::serialize(value).unwrap();
        match bincode::deserialize::<T>(&serialized) {
            Ok(is_ok) => {
                assert_eq!(value, &is_ok);
            }
            Err(err) => {
                println!("Error \n {:?} \n {:?}", value, err);
                println!("\n\n{}", serde_json::to_string_pretty(value).unwrap());
                panic!("Some decoding went wrong!");
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
    struct HeaderMapSerialize {
        #[serde(with = "optional_header_map")]
        value: Option<HeaderMap>,
    }

    #[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
    struct StatusCodeSerialize {
        #[serde(with = "status_code")]
        value: StatusCode,
    }

    fn check_serialisation(data: &CrawlResult) {
        check_serialisation_value(&data.meta.url);
        check_serialisation_value(&data.content);
        check_serialisation_value(&data.meta.file_information);
        check_serialisation_value(&data.meta.final_redirect_destination);
        check_serialisation_value(&data.meta.created_at);
        check_serialisation_value(&data.meta.recognized_encoding);
        check_serialisation_value(&data.meta.links);
        check_serialisation_value(&HeaderMapSerialize {
            value: data.meta.headers.clone(),
        });
        check_serialisation_value(&StatusCodeSerialize {
            value: data.meta.status_code.clone(),
        });
    }

    fn init2() {
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

    #[tokio::test]
    async fn crawl_a_single_site() {
        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::SeedOnly {
            depth_on_website: 1,
            recrawl_interval: Some(Duration::milliseconds(5000)),
            request_timeout: None,
        };

        let context = TestContext::new(
            AtraConfig::new(
                Default::default(),
                Default::default(),
                Default::default(),
                config,
            ),
            FakeClientProvider::new(),
        );

        context.provider().insert(
            "https://www.ebay.com/".parse().unwrap(),
            Ok(
                FakeResponse::new(
                    Some(
                        FetchedRequestData::new(
                            RawData::from_vec(include_bytes!("../../testdata/samples/HTML attribute reference - HTML_ HyperText Markup Language _ MDN.html").to_vec()),
                            None,
                            StatusCode::OK,
                            None,
                            None,
                            false,
                        )
                    ),
                    1,
                )
            ),
        );

        let mut crawl_task = context
            .create_crawl_task(UnguardedSeed::from_url("https://www.ebay.com/").unwrap())
            .unwrap();

        let result = crawl_task
            .run(&context, ShutdownPhantom::<true>, &TestErrorConsumer::new())
            .await;

        println!("{:?}", result);

        drop(crawl_task);
        drop(result);

        context.provider().insert(
            "https://www.ebay.com/test".parse().unwrap(),
            Ok(FakeResponse::new(
                Some(FetchedRequestData::new(
                    RawData::from_vec(
                        include_bytes!("../../testdata/samples/Amazon.html").to_vec(),
                    ),
                    None,
                    StatusCode::OK,
                    None,
                    None,
                    false,
                )),
                1,
            )),
        );

        let mut crawl_task = context
            .create_crawl_task(UnguardedSeed::from_url("https://www.ebay.com/").unwrap())
            .unwrap();

        let result = crawl_task
            .run(&context, ShutdownPhantom::<true>, &TestErrorConsumer::new())
            .await;

        drop(crawl_task);

        println!("{:?}", result);

        println!("{}", context.url_queue().len().await);

        tokio::time::sleep(std::time::Duration::from_millis(6000)).await;

        let x = context.url_queue().dequeue().await.unwrap().unwrap().take();
        println!("{}", context.url_queue().len().await);

        let origin = x.target.atra_origin().unwrap();
        let mut crawl_task = context
            .create_crawl_task(UnguardedSeed::new(x.target, origin, true).unwrap())
            .unwrap();

        let result = crawl_task
            .run(&context, ShutdownPhantom::<true>, &TestErrorConsumer::new())
            .await;

        println!("{:?}", result);

        println!("{}", context.url_queue().len().await);

        let (a, b) = context.get_all_crawled_websites();

        for (k, v) in a {
            println!("{}\n", &k);
            println!("{:?}", &v);
        }
    }

    #[tokio::test]
    async fn crawl_a_single_site_filtered() {
        // // init();
        // let mut config: CrawlConfig = CrawlConfig::default();
        // config.budget.default = BudgetSetting::SeedOnly {
        //     depth_on_website: 2,
        //     recrawl_interval: None,
        //     request_timeout: None,
        // };
        //
        // let mut crawl = CrawlTask::builder(&config)
        //     .build(
        //         UnguardedSeed::new(
        //             "https://choosealicense.com/"
        //                 .parse::<UrlWithDepth>()
        //                 .unwrap(),
        //             "choosealicense.com".into(),
        //         )
        //         .unwrap(),
        //     )
        //     .await;
        //
        // let context = InMemoryContext::with_blacklist(
        //     Configs::new(
        //         Default::default(),
        //         Default::default(),
        //         config,
        //         Default::default(),
        //     ),
        //     ".*github.*"
        //         .parse::<RegexBlackList>()
        //         .expect("Should be able to parse a url.")
        //         .into(),
        // );
        //
        // crawl
        //     .crawl(
        //         &context,
        //         ShutdownPhantom,
        //         &crate::app::consumer::GlobalErrorConsumer::new(),
        //     )
        //     .await
        //     .expect("Expected a positive result!");
    }

    // #[tokio::test]
    // async fn crawl_a_single_site_with_depth() {
    //     init();
    //     let mut config: CrawlConfig = CrawlConfig::default();
    //     config.budget.default = BudgetSetting::Absolute {
    //         depth: 2,
    //         recrawl_interval: None,
    //         request_timeout: None,
    //     };
    //     config.delay = Some(Duration::milliseconds(300));
    //     config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());
    //
    //     log::info!("START");
    //     let context: LocalContext = LocalContext::new(
    //         Configs::new(
    //             Default::default(),
    //             Default::default(),
    //             config,
    //             Default::default(),
    //         ),
    //         RuntimeContext::unbound(),
    //     )
    //         .await
    //         .unwrap();
    //     let config = context.configs().crawl.clone();
    //
    //     context
    //         .url_queue()
    //         .enqueue_seed("https://choosealicense.com/")
    //         .await
    //         .unwrap();
    //     let context = WorkerContext::create(0, Arc::new(context)).await.unwrap();
    //
    //     while !context.url_queue().is_empty().await {
    //         log::trace!("TEST: NEXT");
    //         let guard_with_seed = context.poll_next_free_url(ShutdownPhantom, None).await;
    //         let guard_with_seed = match guard_with_seed {
    //             UrlQueuePollResult::Ok(guard_with_seed) => guard_with_seed,
    //             UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty) => {
    //                 log::error!("Abort: Queue was empty.");
    //                 break;
    //             }
    //             UrlQueuePollResult::Abort(cause) => {
    //                 log::error!("Abort: {cause}");
    //                 break;
    //             }
    //             UrlQueuePollResult::Err(err) => {
    //                 log::error!("Panic: {err}");
    //                 panic!("Had an error: {err:?}")
    //             }
    //         };
    //         log::trace!("Build Task");
    //         let mut crawl_task: WebsiteCrawler<_> = WebsiteCrawlerBuilder::new(&config)
    //             .build(guard_with_seed.get_guarded_seed())
    //             .await;
    //         log::trace!("Crawl Task");
    //         crawl_task
    //             .crawl(
    //                 &context,
    //                 ShutdownPhantom,
    //                 &crate::app::consumer::GlobalErrorConsumer::new(),
    //             )
    //             .await
    //             .unwrap();
    //         log::trace!("Continue");
    //     }
    // }
}
