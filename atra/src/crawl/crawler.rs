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
pub use result::test::*;

use crate::blacklist::lists::BlackList;
use crate::client::{Client, ClientBuilder};
use crate::config::crawl::RedirectPolicy;
use crate::config::{BudgetSetting, CrawlConfig};
use crate::contexts::traits::{
    SupportsBlackList, SupportsConfigs, SupportsCrawlResults, SupportsFileSystemAccess,
    SupportsGdbrRegistry, SupportsLinkSeeding, SupportsLinkState, SupportsRobotsManager,
    SupportsSlimCrawlResults,
};
use crate::crawl::crawler::intervals::InvervalManager;
use crate::crawl::crawler::result::CrawlResult;
use crate::crawl::crawler::sitemaps::retrieve_and_parse;
use crate::data::{process, RawData, RawVecData};
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::AtraFileInformation;
use crate::io::fs::AtraFS;
use crate::link_state::LinkStateType;
use crate::fetching::ResponseData;
use crate::robots::{GeneralRobotsInformation, RobotsInformation};
use crate::runtime::ShutdownReceiver;
use crate::seed::BasicSeed;
use crate::toolkit::detect_language;
use crate::toolkit::domains::domain_name;
use crate::url::{AtraOriginProvider, AtraUrlOrigin, UrlWithDepth};
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use itertools::Itertools;
use log::LevelFilter;
use rand::distributions::Alphanumeric;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Attempt;
use sitemap::structs::Location;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use strum::EnumString;
use time::{Duration, OffsetDateTime};

/// A builder for a crawl task, can be used as template
#[derive(Debug, Clone)]
pub struct WebsiteCrawlerBuilder<'a> {
    /// Configuration properties for website.
    configuration: &'a CrawlConfig,
    /// Set the crawl ID to track. This allows explicit targeting for shutdown, pause, and etc.
    crawl_id: Option<String>,
    /// User agent
    user_agent: Option<String>,
    /// Use HTTP2 for connection. Enable if you know the website has http2 support.
    http2_prior_knowledge: bool,
    /// Additional headers
    additional_headers: Option<HeaderMap>,
    /// The urls to the sitemaps
    sitemaps: Option<HashMap<AtraUrlOrigin, Vec<String>>>,
}

#[allow(dead_code)]
impl<'a> WebsiteCrawlerBuilder<'a> {
    pub fn new(configuration: &'a CrawlConfig) -> Self {
        Self {
            configuration,
            crawl_id: None,
            user_agent: None,
            http2_prior_knowledge: false,
            sitemaps: None,
            additional_headers: None,
        }
    }

    pub fn set_crawl_id(mut self, crawl_id: Option<String>) -> Self {
        self.crawl_id = crawl_id;
        self
    }

    pub fn set_user_agent(mut self, value: Option<String>) -> Self {
        self.user_agent = value;
        self
    }

    pub fn set_http2_prior_knowledge(mut self) -> Self {
        self.http2_prior_knowledge = true;
        self
    }

    pub fn add_additional_header(
        mut self,
        header_name: HeaderName,
        header_value: HeaderValue,
    ) -> Self {
        if let Some(ref mut headers) = self.additional_headers {
            headers.insert(header_name, header_value);
        } else {
            let mut headers = HeaderMap::new();
            headers.insert(header_name, header_value);
            self.additional_headers = Some(headers)
        }
        self
    }

    pub fn add_additional_headers(mut self, header_map: &HeaderMap) -> Self {
        if let Some(ref mut headers) = self.additional_headers {
            headers.extend(header_map.clone());
        } else {
            self.additional_headers = Some(header_map.clone())
        }
        self
    }

    /// Ignores urls that can not provide a domain
    pub fn add_sitemap(mut self, url_with_depth: &UrlWithDepth, value: String) -> Self {
        if let Some(ref mut sitemaps) = self.sitemaps {
            if let Some(host) = url_with_depth.atra_origin() {
                if let Some(found) = sitemaps.get_mut(&host) {
                    found.push(value);
                } else {
                    sitemaps.insert(host, vec![value]);
                }
            }
        } else {
            let mut hash_map = HashMap::new();
            if let Some(host) = url_with_depth.atra_origin() {
                hash_map.insert(host, vec![value]);
            }
            self.sitemaps = Some(hash_map);
        }
        self
    }

    pub fn add_sitemaps<I: IntoIterator<Item = String>>(
        mut self,
        url_with_depth: &UrlWithDepth,
        values: Vec<String>,
    ) -> Self {
        if let Some(ref mut sitemaps) = self.sitemaps {
            if let Some(hosts) = url_with_depth.atra_origin() {
                if let Some(found) = sitemaps.get_mut(&hosts) {
                    found.extend(values);
                } else {
                    sitemaps.insert(hosts, values);
                }
            }
        } else {
            let mut hash_map = HashMap::new();
            if let Some(domain) = url_with_depth.atra_origin() {
                hash_map.insert(domain, values);
            }
            self.sitemaps = Some(hash_map);
        }
        self
    }

    /// Setup redirect policy for reqwest.
    fn setup_redirect_policy(&self, url: &UrlWithDepth) -> reqwest::redirect::Policy {
        match self.configuration.redirect_policy {
            RedirectPolicy::Loose => {
                reqwest::redirect::Policy::limited(self.configuration.redirect_limit)
            }
            RedirectPolicy::Strict => {
                let host_s = url.atra_origin().unwrap_or_default();
                let default_policy = reqwest::redirect::Policy::default();
                let initial_redirect = Arc::new(AtomicU8::new(0));
                let initial_redirect_limit = if self.configuration.respect_robots_txt {
                    2
                } else {
                    1
                };
                let subdomains = self.configuration.subdomains;
                let tld = self.configuration.tld;
                let host_domain_name = if tld {
                    url.domain_name().unwrap_or_default()
                } else {
                    Default::default()
                };
                let redirect_limit = self.configuration.redirect_limit;

                let to_mode = url.clone();

                let custom_policy = {
                    move |attempt: Attempt| {
                        let attempt_url = domain_name(attempt.url()).unwrap_or_default();

                        if tld && attempt_url == host_domain_name
                            || subdomains
                                && attempt
                                    .url()
                                    .host_str()
                                    .unwrap_or_default()
                                    .ends_with(host_s.as_ref())
                            || to_mode.url().same_host_url(&attempt.url())
                        {
                            default_policy.redirect(attempt)
                        } else if attempt.previous().len() > redirect_limit {
                            attempt.error("too many redirects")
                        } else if attempt.status().is_redirection()
                            && (0..initial_redirect_limit)
                                .contains(&initial_redirect.load(Ordering::Relaxed))
                        {
                            initial_redirect.fetch_add(1, Ordering::Relaxed);
                            default_policy.redirect(attempt)
                        } else {
                            attempt.stop()
                        }
                    }
                };
                reqwest::redirect::Policy::custom(custom_policy)
            }
        }
    }

    /// Creates a configured clientbuilder with the provided informations.
    fn configure_http_client_builder<T: BasicSeed>(
        &self,
        seed: &T,
        user_agent: &str,
    ) -> ClientBuilder {
        let mut client = reqwest::Client::builder()
            .user_agent(user_agent)
            .danger_accept_invalid_certs(self.configuration.accept_invalid_certs)
            .tcp_keepalive(Duration::milliseconds(500).unsigned_abs())
            .pool_idle_timeout(None);

        if self.http2_prior_knowledge {
            client = client.http2_prior_knowledge();
        }

        let mut headers_for_client = HeaderMap::with_capacity(
            self.configuration.headers.as_ref().map_or(0, |it| it.len())
                + self.additional_headers.as_ref().map_or(0, |it| it.len()),
        );

        if let Some(ref headers) = self.additional_headers {
            headers_for_client.extend(headers.clone());
        }

        if let Some(ref headers) = self.configuration.headers {
            headers_for_client.extend(headers.clone());
        }

        if !headers_for_client.is_empty() {
            client = client.default_headers(headers_for_client);
        }
        let url = seed.url();

        client = client.redirect(self.setup_redirect_policy(url));
        if let Some(timeout) = self
            .configuration
            .budget
            .get_budget_for(&seed.origin())
            .get_request_timeout()
        {
            log::trace!("Timeout Set: {}", timeout);
            client = client.timeout(timeout.unsigned_abs());
        }
        client = if let Some(cookies) = &self.configuration.cookies {
            if let Some(cookie) = cookies.get_cookies_for(&seed.origin()) {
                let cookie_store = reqwest::cookie::Jar::default();
                if let Some(url) = url.clean_url().as_url() {
                    cookie_store.add_cookie_str(cookie.as_str(), url);
                }
                client.cookie_provider(cookie_store.into())
            } else {
                client.cookie_store(self.configuration.use_cookies)
            }
        } else {
            client.cookie_store(self.configuration.use_cookies)
        };

        if let Some(ref proxies) = self.configuration.proxies {
            for proxy in proxies {
                match reqwest::Proxy::all(proxy) {
                    Ok(proxy) => {
                        client = client.proxy(proxy);
                    }
                    _ => {}
                }
            }
        }

        let mut client = ClientBuilder::new(client.build().unwrap());
        if self.configuration.cache {
            client = client.with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager::default(),
                options: HttpCacheOptions::default(),
            }));
        }

        client
    }

    pub async fn build<T: BasicSeed>(self, seed: T) -> WebsiteCrawler<T> {
        let build_crawl_task_at = OffsetDateTime::now_utc();

        let user_agent = self
            .user_agent
            .clone()
            .unwrap_or_else(|| self.configuration.user_agent.get_user_agent().to_string());
        let client_builder = self.configure_http_client_builder(&seed, &user_agent);

        let client = client_builder.build();

        let crawl_id = self.crawl_id.unwrap_or_else(|| {
            let mut result: String = "crawl".to_string();
            result.push('-');
            result.push_str(
                &data_encoding::BASE32_NOPAD
                    .encode(&build_crawl_task_at.unix_timestamp_nanos().to_be_bytes()),
            );
            result.push('-');
            result.push_str(
                &rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(15)
                    .map(char::from)
                    .collect::<String>(),
            );
            result
        });

        WebsiteCrawler::new(
            seed,
            build_crawl_task_at,
            crawl_id,
            user_agent,
            client,
            self.sitemaps,
        )
    }
}

/// A crawler for a single website. Starts from the provided `seed` and
#[derive(Debug)]
pub struct WebsiteCrawler<S> {
    /// The seed of the crawl task
    seed: S,
    /// When was the crawl task built?
    #[allow(dead_code)]
    was_build_at: OffsetDateTime,
    /// User agent
    user_agent: String,
    /// The request client. Stored for re-use between runs.
    client: Client,

    /// All URLs visited.
    links_visited: HashSet<UrlWithDepth>,

    /// Set the crawl ID to track. This allows explicit targeting for shutdown, pause, and etc.
    #[allow(dead_code)]
    pub crawl_id: String,

    /// External sitemaps set by the builder
    #[allow(dead_code)]
    external_sitemaps: Option<HashMap<AtraUrlOrigin, Vec<String>>>,
}

impl<S: BasicSeed> WebsiteCrawler<S> {
    /// Creates a new instance of a WebsiteCrawler
    fn new(
        seed: S,
        was_build_at: OffsetDateTime,
        crawl_id: String,
        user_agent: String,
        client: Client,
        external_sitemaps: Option<HashMap<AtraUrlOrigin, Vec<String>>>,
    ) -> Self {
        Self {
            seed,
            was_build_at,
            crawl_id,
            links_visited: HashSet::new(),
            user_agent,
            client,
            external_sitemaps,
        }
    }
}

impl<'a> WebsiteCrawler<WebsiteCrawlerBuilder<'a>> {
    #[cfg(test)]
    pub fn builder(crawl_config: &'a CrawlConfig) -> WebsiteCrawlerBuilder<'a> {
        WebsiteCrawlerBuilder::new(crawl_config)
    }
}

impl<S: BasicSeed> WebsiteCrawler<S> {
    async fn update_linkstate<C, E>(
        handler: &mut Vec<E>,
        context: &C,
        target: &UrlWithDepth,
        link_state_type: LinkStateType,
    ) -> bool
    where
        C: SupportsLinkState,
        E: From<<C as SupportsLinkState>::Error>,
    {
        match context.update_link_state(target, link_state_type).await {
            Ok(_) => true,
            Err(error) => {
                handler.push(error.into());
                false
            }
        }
    }

    async fn pack_shutdown<C, E>(
        mut handler: Vec<E>,
        context: &C,
        target: &UrlWithDepth,
        link_state_type: LinkStateType,
    ) -> Result<Option<Vec<E>>, Vec<E>>
    where
        C: SupportsLinkState,
        E: From<<C as SupportsLinkState>::Error>,
    {
        if !Self::update_linkstate(&mut handler, context, &target, link_state_type).await {
            log::info!("Failed to set link state of {target}, continue shutdown.");
        }
        return if handler.is_empty() {
            Ok(Some(handler))
        } else {
            log::trace!("Shutdown with errors");
            Err(handler)
        };
    }

    /// The crawl method.
    pub async fn crawl<Cont, Shutdown, E>(
        &mut self,
        context: &Cont,
        shutdown: Shutdown,
    ) -> Result<Option<Vec<E>>, Vec<E>>
    where
        Cont: SupportsGdbrRegistry
            + SupportsConfigs
            + SupportsRobotsManager
            + SupportsBlackList
            + SupportsLinkState
            + SupportsSlimCrawlResults
            + SupportsFileSystemAccess
            + SupportsCrawlResults
            + SupportsLinkSeeding,
        Shutdown: ShutdownReceiver,
        E: From<<Cont as SupportsSlimCrawlResults>::Error>
            + From<<Cont as SupportsLinkSeeding>::Error>
            + From<<Cont as SupportsCrawlResults>::Error>
            + From<<Cont as SupportsLinkState>::Error>
            + From<crate::client::ClientError>
            + From<io::Error>
            + Display,
    {
        let configuration = context.configs().crawl();

        if shutdown.is_shutdown() {
            return Ok(None);
        }

        log::debug!("Start Crawling of {}", self.seed.url());

        let configured_robots = Arc::new(
            GeneralRobotsInformation::new(
                context.get_robots_instance().await,
                self.user_agent.clone(),
                configuration.max_robots_age.clone(),
            )
            .bind_to_domain(&self.client, self.seed.url())
            .await,
        );

        let budget = configuration
            .budget
            .get_budget_for(&self.seed.origin())
            .clone();

        let blacklist = context.get_blacklist().await;

        log::debug!("Local blacklist initialized {:}", self.seed.url());
        let mut queue = VecDeque::with_capacity(128);

        queue.push_back(self.seed.url().clone());

        let mut handler = Vec::new();

        match context.register_seed(&self.seed).await {
            Ok(_) => {}
            Err(err) => {
                handler.push(err.into());
            }
        }

        let checker = UrlChecker {
            configured_robots: configured_robots.as_ref(),
            blacklist: &blacklist,
            budget: &budget,
        };

        // todo: do not ignore sitemaps?

        let mut interval_manager =
            InvervalManager::new(&self.client, configuration, configured_robots.clone());

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
                            queue.push_back(url);
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

        while let Some(target) = queue.pop_front() {
            if shutdown.is_shutdown() {
                return Ok(Some(handler));
            }
            log::trace!("Queue.len() => {}", queue.len());
            if !checker.check_if_allowed(self, &target).await {
                log::debug!("Dropped: {}", target);
                continue;
            }

            if !Self::update_linkstate(
                &mut handler,
                context,
                &target,
                LinkStateType::ReservedForCrawl,
            )
            .await
            {
                log::error!(
                    "Failed setting of linkstate of {target}, continue without further processing."
                );
                continue;
            }

            match context.retrieve_slim_crawled_website(&target).await {
                Ok(value) => {
                    if let Some(already_crawled) = value {
                        if let Some(recrawl) = configuration
                            .budget
                            .get_budget_for(&self.seed.origin())
                            .get_recrawl_interval()
                        {
                            if OffsetDateTime::now_utc() - already_crawled.meta.created_at
                                >= recrawl
                            {
                                log::debug!("The url was already crawled.");
                                if !Self::update_linkstate(
                                    &mut handler,
                                    context,
                                    &target,
                                    LinkStateType::ProcessedAndStored,
                                )
                                .await
                                {
                                    log::info!(
                                        "Failed set correct linkstate of {target}, ignoring."
                                    );
                                }
                                continue;
                            }
                        } else {
                            log::debug!("The url was already crawled.");
                            if !Self::update_linkstate(
                                &mut handler,
                                context,
                                &target,
                                LinkStateType::ProcessedAndStored,
                            )
                            .await
                            {
                                log::info!("Failed set correct linkstate of {target}, ignoring.");
                            }
                            continue;
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
                return Self::pack_shutdown(handler, context, &target, LinkStateType::Discovered)
                    .await;
            }
            if log::max_level() == LevelFilter::Trace {
                log::trace!("Interval Start: {}", OffsetDateTime::now_utc());
            }
            interval_manager.wait(&target).await;
            if log::max_level() == LevelFilter::Trace {
                log::trace!("Interval End: {}", OffsetDateTime::now_utc());
            }
            let url_str = target.as_str().into_owned();
            match crate::fetching::fetch_request(context, &self.client, &url_str).await {
                Ok(page) => {
                    if !Self::update_linkstate(
                        &mut handler,
                        context,
                        &target,
                        LinkStateType::Discovered,
                    )
                    .await
                    {
                        log::info!("Failed to set link state of {target}.");
                    }

                    log::trace!("Fetched: {}", target);
                    let mut response_data = ResponseData::new(page, target.clone());

                    let file_information = AtraFileInformation::determine(context, &response_data);

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
                                    .crawl()
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
                            handler,
                            context,
                            &target,
                            LinkStateType::Discovered,
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
                                        queue.push_back(in_seed);
                                    } else {
                                        log::debug!("Dropped: {in_seed}");
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to handle links with {err}. Stopping crawl.");
                                handler.push(err.into());
                                return Self::pack_shutdown(
                                    handler,
                                    context,
                                    &target,
                                    LinkStateType::Discovered,
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
                            handler,
                            context,
                            &target,
                            LinkStateType::Discovered,
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
                            handler.push(err.into());
                            return Self::pack_shutdown(
                                handler,
                                context,
                                &target,
                                LinkStateType::Discovered,
                            )
                            .await;
                        }
                        _ => {
                            log::debug!("Stored: {}", result.meta.url);
                        }
                    }

                    if !Self::update_linkstate(
                        &mut handler,
                        context,
                        &target,
                        LinkStateType::ProcessedAndStored,
                    )
                    .await
                    {
                        log::error!("Failed setting of linkstate of {target}.");
                    }
                }
                Err(err) => {
                    log::warn!("Failed to fetch {} with error {}", target, err);

                    if !Self::update_linkstate(
                        &mut handler,
                        context,
                        &target,
                        LinkStateType::InternalError,
                    )
                    .await
                    {
                        log::error!("Failed recovery of linkstate of {target}.");
                    }
                }
            }
        }
        Ok((!handler.is_empty()).then_some(handler))
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

struct UrlChecker<'a, R: RobotsInformation, B: BlackList> {
    budget: &'a BudgetSetting,
    configured_robots: &'a R,
    blacklist: &'a B,
}

impl<'a, R: RobotsInformation, B: BlackList> UrlChecker<'a, R, B> {
    /// return `true` if link:
    ///
    /// - is not already crawled
    /// - is not over crawl budget
    /// - is not blacklisted
    /// - is not forbidden in robot.txt file (if parameter is defined)
    async fn check_if_allowed<T: BasicSeed>(
        &self,
        task: &WebsiteCrawler<T>,
        url: &UrlWithDepth,
    ) -> bool {
        let result = !task.links_visited.contains(url)
            && !self.blacklist.has_match_for(&url.as_str())
            && self
                .configured_robots
                .check_if_allowed(&task.client, url)
                .await
            && self.budget.is_in_budget(url);

        match log::max_level() {
            LevelFilter::Trace => {
                let reason = {
                    let mut reasons = SmallVec::<[NotAllowedReasoning; 4]>::new();
                    if task.links_visited.contains(url) {
                        reasons.push(NotAllowedReasoning::IsAlreadyVisited);
                    }
                    if self.blacklist.has_match_for(&url.as_str()) {
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
}

#[cfg(test)]
mod test {
    use crate::blacklist::lists::RegexBlackList;
    use crate::config::crawl::UserAgent;
    use crate::config::{BudgetSetting, Configs, CrawlConfig};
    use crate::contexts::local::LocalContext;
    use crate::contexts::traits::{SupportsConfigs, SupportsPolling, SupportsUrlQueue};
    use crate::contexts::worker::WorkerContext;
    use crate::crawl::crawler::{WebsiteCrawler, WebsiteCrawlerBuilder};
    use crate::crawl::CrawlResult;
    use crate::queue::polling::{AbortCause, UrlQueuePollResult};
    use crate::robots::{InMemoryRobotsManager, RobotsManager};
    use crate::runtime::{RuntimeContext, ShutdownPhantom};
    use crate::seed::UnguardedSeed;
    use crate::test_impls::InMemoryContext;
    use crate::url::queue::UrlQueue;
    use crate::url::UrlWithDepth;
    use itertools::Itertools;
    use log::LevelFilter;
    use log4rs::append::file::FileAppender;
    use log4rs::config::{Appender, Config, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use reqwest::header::HeaderMap;
    use reqwest::StatusCode;
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;
    use std::sync::Arc;
    use time::Duration;
    use crate::toolkit::header_map_extensions::optional_header_map;
    use crate::toolkit::serde_ext::status_code;

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

    #[tokio::test]
    async fn can_use_default_client() {
        let seed = "https://choosealicense.com/"
            .parse::<UrlWithDepth>()
            .unwrap()
            .try_into()
            .unwrap();
        let crawl_task: WebsiteCrawler<UnguardedSeed> =
            WebsiteCrawler::builder(&CrawlConfig::default())
                .build(seed)
                .await;
        let in_memory = InMemoryRobotsManager::new();
        println!("{:?}", crawl_task.client);
        let retrieved = in_memory
            .get_or_retrieve(
                &crawl_task.client,
                &crawl_task.user_agent,
                &"https://choosealicense.com/"
                    .parse::<UrlWithDepth>()
                    .unwrap(),
                None,
            )
            .await;
        println!("{:?}", retrieved)
    }

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

    #[tokio::test]
    async fn crawl_a_single_site() {
        init();

        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::SeedOnly {
            depth_on_website: 3,
            recrawl_interval: None,
            request_timeout: None,
        };

        log::info!("START");

        let mut crawl = WebsiteCrawler::builder(&config)
            .build(
                UnguardedSeed::new(
                    "https://choosealicense.com/"
                        .parse::<UrlWithDepth>()
                        .unwrap(),
                    "choosealicense.com".into(),
                )
                .unwrap(),
            )
            .await;

        let context = InMemoryContext::new(Configs::new(
            Default::default(),
            Default::default(),
            config,
            Default::default(),
        ));

        crawl
            .crawl::<_, _, anyhow::Error>(&context, ShutdownPhantom)
            .await
            .expect("Expected a positive result!");

        // for ref crawled in crawled {
        //     let found = context.get_crawled_website(crawled).await.expect("The website should be working!");
        //     println!("--------\n{:?}", found.map(|value| value.url.to_string()).unwrap_or_default())
        // }

        let (a, _) = context.get_all_crawled_websites();
        let values = a.into_values().collect_vec();

        for value in &values {
            println!("--------\n{:?}", &value.meta.url.to_string());
            // if value.url.is_same_url_as("")
            check_serialisation(value);
        }

        // File::create("./testdata/testdata.bin").unwrap().write_all(
        //     &bincode::serialize(&values).unwrap()
        // ).unwrap();
    }

    #[tokio::test]
    async fn crawl_a_single_site_filtered() {
        // init();
        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::SeedOnly {
            depth_on_website: 2,
            recrawl_interval: None,
            request_timeout: None,
        };

        let mut crawl = WebsiteCrawler::builder(&config)
            .build(
                UnguardedSeed::new(
                    "https://choosealicense.com/"
                        .parse::<UrlWithDepth>()
                        .unwrap(),
                    "choosealicense.com".into(),
                )
                .unwrap(),
            )
            .await;

        let context = InMemoryContext::with_blacklist(
            Configs::new(
                Default::default(),
                Default::default(),
                config,
                Default::default(),
            ),
            ".*github.*"
                .parse::<RegexBlackList>()
                .expect("Should be able to parse a url.")
                .into(),
        );

        crawl
            .crawl::<_, _, anyhow::Error>(&context, ShutdownPhantom)
            .await
            .expect("Expected a positive result!");
    }

    #[tokio::test]
    async fn crawl_a_single_site_with_depth() {
        init();
        let mut config: CrawlConfig = CrawlConfig::default();
        config.budget.default = BudgetSetting::Absolute {
            depth: 2,
            recrawl_interval: None,
            request_timeout: None,
        };
        config.delay = Some(Duration::milliseconds(300));
        config.user_agent = UserAgent::Custom("TestCrawl/Atra/v0.1.0".to_string());

        log::info!("START");
        let context: LocalContext = LocalContext::new(
            Configs::new(
                Default::default(),
                Default::default(),
                config,
                Default::default(),
            ),
            RuntimeContext::unbound(),
        )
        .await
        .unwrap();
        let config = context.configs().crawl.clone();

        context
            .url_queue()
            .enqueue_seed("https://choosealicense.com/")
            .await
            .unwrap();
        let context = WorkerContext::create(0, Arc::new(context)).await.unwrap();

        while !context.url_queue().is_empty().await {
            log::trace!("TEST: NEXT");
            let guard_with_seed = context.poll_next_free_url(ShutdownPhantom, None).await;
            let guard_with_seed = match guard_with_seed {
                UrlQueuePollResult::Ok(guard_with_seed) => guard_with_seed,
                UrlQueuePollResult::Abort(AbortCause::QueueIsEmpty) => {
                    log::error!("Abort: Queue was empty.");
                    break;
                }
                UrlQueuePollResult::Abort(cause) => {
                    log::error!("Abort: {cause}");
                    break;
                }
                UrlQueuePollResult::Err(err) => {
                    log::error!("Panic: {err}");
                    panic!("Had an error: {err:?}")
                }
            };
            log::trace!("Build Task");
            let mut crawl_task: WebsiteCrawler<_> = WebsiteCrawlerBuilder::new(&config)
                .build(guard_with_seed.get_guarded_seed())
                .await;
            log::trace!("Crawl Task");
            crawl_task.crawl::<_, _, anyhow::Error>(&context, ShutdownPhantom).await.unwrap();
            log::trace!("Continue");
        }
    }
}
