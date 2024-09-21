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

use crate::config::crawl::RedirectPolicy;
use crate::config::Config;
use crate::contexts::traits::{SupportsConfigs, SupportsCrawling};
use crate::seed::BasicSeed;
use crate::toolkit::domains::domain_name;
use crate::url::{AtraOriginProvider, UrlWithDepth};
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::redirect::Attempt;
use reqwest::Error;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use time::Duration;

/// Builds the classic configured client used by Atra
pub fn build_classic_client<C: SupportsCrawling, T: BasicSeed>(
    context: &C,
    seed: &T,
    useragent: impl AsRef<str>,
) -> Result<ClientWithMiddleware, Error>
where
    C: SupportsCrawling + SupportsConfigs,
    T: BasicSeed,
{
    let configs = context.configs();

    let mut client = reqwest::Client::builder()
        .user_agent(useragent.as_ref())
        .danger_accept_invalid_certs(configs.crawl.accept_invalid_certs)
        .tcp_keepalive(Duration::milliseconds(500).unsigned_abs())
        .pool_idle_timeout(None);

    //todo
    // http2_prior_knowledge

    if let Some(ref headers) = configs.crawl.headers {
        client = client.default_headers(headers.clone());
    }

    let url = seed.url();

    client = client.redirect(setup_redirect_policy(configs, url));

    if let Some(timeout) = configs
        .crawl
        .budget
        .get_budget_for(&seed.origin())
        .get_request_timeout()
    {
        log::trace!("Timeout Set: {}", timeout);
        client = client.timeout(timeout.unsigned_abs());
    }

    client = if let Some(cookies) = &configs.crawl.cookies {
        if let Some(cookie) = cookies.get_cookies_for(&seed.origin()) {
            let cookie_store = reqwest::cookie::Jar::default();
            if let Some(url) = url.clean_url().as_url() {
                cookie_store.add_cookie_str(cookie.as_str(), url);
            }
            client.cookie_provider(cookie_store.into())
        } else {
            client.cookie_store(configs.crawl.use_cookies)
        }
    } else {
        client.cookie_store(configs.crawl.use_cookies)
    };

    if let Some(ref proxies) = configs.crawl.proxies {
        for proxy in proxies {
            match reqwest::Proxy::all(proxy) {
                Ok(proxy) => {
                    client = client.proxy(proxy);
                }
                _ => {}
            }
        }
    }

    let mut client = ClientBuilder::new(client.build()?);
    if configs.crawl.cache {
        client = client.with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: CACacheManager::default(),
            options: HttpCacheOptions::default(),
        }));
    }

    Ok(client.build())
}

fn setup_redirect_policy(config: &Config, url: &UrlWithDepth) -> reqwest::redirect::Policy {
    match config.crawl.redirect_policy {
        RedirectPolicy::Loose => reqwest::redirect::Policy::limited(config.crawl.redirect_limit),
        RedirectPolicy::Strict => {
            let host_s = url.atra_origin().unwrap_or_default();
            let default_policy = reqwest::redirect::Policy::default();
            let initial_redirect = Arc::new(AtomicU8::new(0));
            let initial_redirect_limit = if config.crawl.respect_robots_txt {
                2
            } else {
                1
            };
            let subdomains = config.crawl.subdomains;
            let tld = config.crawl.tld;
            let host_domain_name = if tld {
                url.domain_name().unwrap_or_default()
            } else {
                Default::default()
            };
            let redirect_limit = config.crawl.redirect_limit;

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
