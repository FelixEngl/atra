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

use crate::config::crawl::{CookieSettings, CrawlBudget, RedirectPolicy, UserAgent};
use crate::config::{BudgetSetting, CrawlConfig, SessionConfig};
use crate::extraction::extractor::Extractor;
use crate::gdbr::identifier::{
    FilterMode, GdbrIdentifierConfig, GdbrIdentifierRegistryConfig,
    LanguageBoundGdbrIdentifierConfig,
};
use isolang::Language;
use liblinear::parameter::serde::GenericParameters;
use reqwest::header::{HeaderMap, CONTENT_LENGTH, HOST};
use rust_stemmers::Algorithm;
use std::collections::HashMap;
use std::num::NonZeroU64;
use svm::config::{DocumentClassifierConfig, SvmRecognizerConfig};
use text_processing::configs::StopwordRegistryConfig;
use text_processing::stopword_registry::StopWordRepository;
use time::Duration;
use ubyte::ToByteUnit;

pub const ATRA_LOGO: &'static str = include_str!("logo_small.txt");
pub const ATRA_WELCOME: &'static str = include_str!("welcome.txt");

pub fn create_example_config() -> crate::config::configs::Config {
    crate::config::configs::Config {
        system: Default::default(),
        paths: Default::default(),
        session: SessionConfig {
            service: "My Service".to_string(),
            collection: "MyCollection".to_string(),
            crawl_job_id: 0,
        },
        crawl: CrawlConfig {
            user_agent: UserAgent::Custom("My User Agent".to_string()),
            respect_robots_txt: true,
            respect_nofollow: true,
            crawl_forms: false,
            crawl_embedded_data: false,
            crawl_javascript: true,
            crawl_onclick_by_heuristic: true,
            apply_gdbr_filter_if_possible: false,
            store_only_html_in_warc: true,
            store_big_file_hints_in_warc: true,
            max_file_size: Some(NonZeroU64::new(1.gigabytes().as_u64()).unwrap()),
            max_robots_age: Some(Duration::seconds(60 * 24)),
            ignore_sitemap: false,
            subdomains: false,
            cache: true,
            use_cookies: true,
            generate_web_graph: true,
            cookies: Some(CookieSettings {
                default: Some("My Default cookie".to_string()),
                per_host: Some({
                    let mut hm = HashMap::new();
                    hm.insert(
                        "google.de".to_string().into(),
                        "My special google cookie".to_string(),
                    );
                    hm
                }),
            }),
            headers: Some({
                let mut hm = HeaderMap::new();
                hm.insert(HOST, "example. com".parse().unwrap());
                hm.insert(CONTENT_LENGTH, "123".parse().unwrap());
                hm
            }),
            proxies: Some(vec!["myproxie.com".to_string()]),
            tld: false,
            delay: Some(Duration::seconds(10)),
            budget: CrawlBudget {
                default: BudgetSetting::Normal {
                    depth: 2,
                    recrawl_interval: None,
                    depth_on_website: 9,
                    request_timeout: Some(Duration::seconds(1)),
                },
                per_host: Some({
                    let mut hm = HashMap::new();
                    hm.insert(
                        "amazon.com".to_string().into(),
                        BudgetSetting::Absolute {
                            depth: 2,
                            request_timeout: Some(Duration::seconds(10)),
                            recrawl_interval: Some(Duration::weeks(1)),
                        },
                    );

                    hm.insert(
                        "microsoft.com".to_string().into(),
                        BudgetSetting::SeedOnly {
                            depth_on_website: 6,
                            request_timeout: Some(Duration::seconds(10)),
                            recrawl_interval: Some(Duration::weeks(1)),
                        },
                    );
                    hm
                }),
            },
            max_queue_age: 30,
            redirect_limit: 5,
            redirect_policy: RedirectPolicy::Loose,
            accept_invalid_certs: true,
            link_extractors: Extractor::default(),
            decode_big_files_up_to: Some(1.gigabytes().as_u64()),
            stopword_registry: Some(StopwordRegistryConfig {
                registries: vec![
                    StopWordRepository::IsoDefault,
                    StopWordRepository::DirRepo {
                        with_iso_default: false,
                        dir: "path/to/my/dir/with/stopwords".parse().unwrap(),
                    },
                    StopWordRepository::File {
                        with_iso_default: true,
                        language: Language::Deu,
                        file: "pyth/to/my/file/with/stopwords.txt".parse().unwrap(),
                    },
                ],
            }),
            gbdr: Some(GdbrIdentifierRegistryConfig {
                default: Some(GdbrIdentifierConfig {
                    threshold: 0.1,
                    filter_threshold: 0.5,
                    filter_by: FilterMode::OnScore,
                    svm: SvmRecognizerConfig::All {
                        language: Language::Deu,
                        min_doc_length: Some(5),
                        min_vector_length: Some(5),
                        retrain_if_possible: true,
                        trained_svm: "path/where/my/svm/is/stored.bin".parse().unwrap(),
                        test_data: None,
                        classifier: DocumentClassifierConfig {
                            min_vector_length: 5,
                            min_doc_length: 5,
                            tf: text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.tf,
                            idf: text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.idf,
                            train_data: "pyth/to/my/train/data/svm.csv".into(),
                            stemmer: Some(Algorithm::English),
                            filter_stopwords: true,
                            tf_idf_data: Some("pyth/to/my/train/data/tf_idf.txt".into()),
                            normalize_tokens: true,
                            parameters: Some(GenericParameters {
                                epsilon: Some(0.0003),
                                p: Some(0.1),
                                cost: Some(10.0),
                                ..GenericParameters::default()
                            }),
                        },
                    },
                }),
                by_language: Some({
                    let mut hm = HashMap::new();
                    hm.insert(
                            Language::Eng,
                            LanguageBoundGdbrIdentifierConfig {
                                required_reliability: 0.8,
                                identifier: GdbrIdentifierConfig{
                                    threshold: 0.1,
                                    filter_threshold: 0.5,
                                    filter_by: FilterMode::OnScore,
                                    svm: SvmRecognizerConfig::All {
                                        language: Language::Deu,
                                        min_doc_length: Some(5),
                                        min_vector_length: Some(5),
                                        retrain_if_possible: true,
                                        trained_svm: "path/where/my/svm/is/stored.bin".parse().unwrap(),
                                        test_data: None,
                                        classifier: DocumentClassifierConfig {
                                            min_vector_length: 5,
                                            min_doc_length: 5,
                                            tf: text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.tf,
                                            idf: text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.idf,
                                            train_data: "pyth/to/my/train/data/svm.csv".into(),
                                            stemmer: Some(Algorithm::German),
                                            filter_stopwords: true,
                                            tf_idf_data: Some("pyth/to/my/train/data/tf_idf.tct".into()),
                                            normalize_tokens:true,
                                            parameters: Some(
                                                GenericParameters {
                                                    epsilon: Some(0.0003),
                                                    p: Some(0.1),
                                                    cost: Some(10.0),
                                                    ..GenericParameters::default()
                                                }
                                            )
                                        }
                                    }
                                }
                            }
                        );
                    hm
                }),
            }),
        },
    }
}
