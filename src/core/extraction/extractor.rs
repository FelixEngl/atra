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

use std::cmp::Ordering;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use enum_iterator::{all};
use crate::core::contexts::Context;
use super::ExtractedLink;
use crate::core::data_processing::{ProcessedData};
use crate::core::extraction::extractor_method::ExtractorMethod;

/*
    To register a new extractor, create a extractor_decode_action_declaration
    and extractor_sub_extractor_declaration.
*/

/// A struct acting as an extractor
#[derive(Debug, Serialize, Deserialize, Clone, Eq)]
#[repr(transparent)]
pub struct Extractor(pub Vec<ExtractorCommand>);

impl PartialEq for Extractor {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|value| other.0.contains(value))
    }
}

impl Extractor {

    /// If the
    async fn apply_extractors<const FALLBACK_MODE: bool>(&self, context: &impl Context, page: &ProcessedData<'_>, result: &mut ExtractorResult) {
        for extractor in &self.0 {
            // Require that both are either true or false
            if FALLBACK_MODE ^ extractor.is_fallback() {
                continue
            }
            if extractor.can_apply(context, page) {
                if result.apply_extractor(extractor.extractor_method) {
                    match extractor.extractor_method.extract_links(context, page, result).await {
                        Ok(value) => {
                            log::debug!("Extracted {value} links with {extractor:?}.");
                        }
                        Err(_) => {
                            log::error!("Failed the extractor {:?}", extractor);
                        }
                    }
                } else {
                    log::debug!("Can not apply extractor because it was already used!")
                }
            } else {
                log::debug!("{extractor:?} is not compatible with the data!")
            }
        }
    }

    /// Extracts the data this the set extractors
    pub async fn extract(&self, context: &impl Context, page: &ProcessedData<'_>) -> ExtractorResult {
        log::trace!("Extractor: {:?} - {}", page.0.page_type, page.0.data.url);
        let mut result = ExtractorResult::default();
        self.apply_extractors::<false>(context, page, &mut result).await;
        if result.no_extractor_applied() || result.is_empty() {
            if !result.no_extractor_applied() {
                log::debug!("Extractor: Unsupported type: {:?}", page.0.page_type);
            }
            self.apply_extractors::<true>(context, page, &mut result).await;
        }
        result
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self(
            all::<ExtractorMethod>()
                .map(|value| ExtractorCommand::new_default_apply(value))
                .collect()
        )
    }
}


/// The result of an extraction, contains the extracted links as well es the applied extractors.
#[derive(Debug, Default)]
pub struct ExtractorResult {
    pub links: HashSet<ExtractedLink>,
    pub applied_extractors: HashSet<ExtractorMethod>
}

impl ExtractorResult {

    /// Returns true if the extractor can be applied
    pub fn apply_extractor(&mut self, extractor: ExtractorMethod) -> bool {
        self.applied_extractors.insert(extractor)
    }

    pub fn register_link(&mut self, link: ExtractedLink) -> bool {
        self.links.insert(link)
    }

    /// Returns true of there are no extracted links
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Returns true if there where no extractors applied.
    pub fn no_extractor_applied(&self) -> bool {
        self.applied_extractors.is_empty()
    }

    /// Converts the result to an optional hashset
    pub fn to_optional_links(self) -> Option<HashSet<ExtractedLink>> {
        if self.is_empty() {
            None
        } else {
            Some(self.links)
        }
    }
}



/// When to apply the extractor?
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, EnumString, Display, Ord, PartialOrd, Eq, PartialEq)]
pub enum ApplyWhen {
    Always,
    #[default]
    IfSuitable,
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct ExtractorCommand {
    extractor_method: ExtractorMethod,
    apply_when: ApplyWhen,
}

impl ExtractorCommand {
    pub fn new(
        extractor_method: ExtractorMethod,
        apply_when: ApplyWhen,
    ) -> Self {
        Self {
            extractor_method,
            apply_when
        }
    }

    pub fn new_default_apply(
        extractor_method: ExtractorMethod,
    ) -> Self {
        Self::new(extractor_method, Default::default())
    }

    pub fn can_apply(&self, context: &impl Context, page: &ProcessedData<'_>) -> bool {
        match self.apply_when {
            ApplyWhen::Always => {true}
            ApplyWhen::IfSuitable => {
                self.extractor_method.is_compatible(context, page)
            }
            ApplyWhen::Fallback => {
                false
            }
        }
    }

    pub fn is_fallback(&self) -> bool {
        return self.apply_when == ApplyWhen::Fallback
    }
}

impl AsRef<ApplyWhen> for ExtractorCommand {
    fn as_ref(&self) -> &ApplyWhen {
        &self.apply_when
    }
}

impl PartialEq<Self> for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn eq(&self, #[as_ref] other: &Self) -> bool;
        }
    }
}

impl PartialOrd<Self> for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn partial_cmp(&self, #[as_ref] other: &Self) -> Option<Ordering>;
        }
    }
}

impl Ord for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn cmp(&self, #[as_ref] other: &Self) -> Ordering;
        }
    }
}


#[cfg(test)]
mod test {
    use crate::core::config::CrawlConfig;
    use crate::core::response::{ResponseData};
    use crate::core::data_processing::process;
    use crate::core::{DataHolder};
    use crate::core::contexts::inmemory::InMemoryContext;
    use crate::core::extraction::extractor::Extractor;
    use crate::core::fetching::{FetchedRequestData};
    use crate::core::UrlWithDepth;

    #[tokio::test]
    async fn can_extract_data() {
        let page = ResponseData::new(
            FetchedRequestData::new(
                DataHolder::from_vec(include_bytes!("../samples/HTML attribute reference - HTML_ HyperText Markup Language _ MDN.html").to_vec()),
                None,
                reqwest::StatusCode::OK,
                None,
                None,
                false
            ),
            UrlWithDepth::from_seed("https://www.example.com/").unwrap()
        );

        let context = InMemoryContext::default();

        let preprocessed = process(&context, &page).await.unwrap();

        let mut cfg: CrawlConfig = Default::default();
        cfg.respect_nofollow = true;
        cfg.crawl_embedded_data = true;



        let extracted = Extractor::default().extract(&context, &preprocessed).await.to_optional_links().unwrap();

        println!("{}", extracted.len());

        for link in extracted {
            println!("{}", link);
        }
    }
}

