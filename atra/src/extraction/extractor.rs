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

use super::ExtractedLink;
use crate::contexts::traits::{SupportsConfigs, SupportsGdbrRegistry};
use crate::data::{Decoded, RawVecData};
use crate::extraction::extractor_method::ExtractorMethod;
use crate::fetching::ResponseData;
use crate::format::AtraFileInformation;
use crate::toolkit::LanguageInformation;
use camino::Utf8PathBuf;
use enum_iterator::all;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use strum::{Display, EnumString};
use crate::url::UrlWithDepth;
/*
    To register a new extractor, create a extractor_decode_action_declaration
    and extractor_sub_extractor_declaration.
*/

/// Wrapps multiple extractor commands to an extractor.
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
    async fn apply_extractors<const FALLBACK_MODE: bool, C>(
        &self,
        context: &C,
        data: ProcessedData<'_>,
        result: &mut ExtractorResult,
    ) where
        C: SupportsConfigs + SupportsGdbrRegistry,
    {
        for extractor in &self.0 {
            // Require that both are either true or false
            if FALLBACK_MODE ^ extractor.is_fallback() {
                continue;
            }
            if extractor.can_apply(&data) {
                if result.apply_extractor(extractor.extractor_method) {
                    match extractor
                        .extractor_method
                        .extract_links(context, &data, result)
                        .await
                    {
                        Ok(value) => {
                            log::debug!("Extracted {value} links with {extractor}.");
                        }
                        Err(err) => {
                            log::warn!(
                                "Failed {extractor} for {} {} with: {}",
                                data.url.url,
                                data.file_info,
                                err
                            );
                        }
                    }
                } else {
                    log::debug!("Can not apply {extractor} because it was already used!")
                }
            } else {
                log::debug!(
                    "{extractor} is not compatible with {} {}!",
                    data.url.url,
                    data.file_info
                )
            }
        }
    }

    /// Extracts the data this the set extractors
    pub async fn extract<C>(
        &self,
        context: &C,
        response: &ResponseData,
        identified_type: &AtraFileInformation,
        decoded: &Decoded<String, Utf8PathBuf>,
        lang: Option<&LanguageInformation>,
    ) -> ExtractorResult
    where
        C: SupportsConfigs + SupportsGdbrRegistry,
    {
        log::trace!("Extractor: {:?} - {}", identified_type.format, response.url);
        let mut result = ExtractorResult::default();
        let holder = ProcessedData::new_from_response(response, identified_type, decoded, lang);
        self.apply_extractors::<false, _>(context, holder, &mut result)
            .await;
        if result.no_extractor_applied() || result.is_empty() {
            if !result.no_extractor_applied() {
                log::debug!("Extractor: Unsupported type: {:?}", identified_type.format);
            }
            self.apply_extractors::<true, _>(context, holder, &mut result)
                .await;
        }
        result
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self(
            all::<ExtractorMethod>()
                .map(|value| ExtractorCommand::new_default_apply(value))
                .collect(),
        )
    }
}

/// A reference to all contents available to extract the data.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ProcessedData<'a> {
    pub url: &'a UrlWithDepth,
    pub raw_data: &'a RawVecData,
    pub file_info: &'a AtraFileInformation,
    pub decoded: &'a Decoded<String, Utf8PathBuf>,
    pub language: Option<&'a LanguageInformation>,
}

impl<'a> ProcessedData<'a> {
    pub fn new_from_response(
        data: &'a ResponseData,
        file_info: &'a AtraFileInformation,
        decoded: &'a Decoded<String, Utf8PathBuf>,
        language: Option<&'a LanguageInformation>,
    ) -> Self {
        Self {
            url: &data.url,
            raw_data: &data.content,
            file_info,
            decoded,
            language
        }
    }
}

/// The result of an extraction, contains the extracted links as well es the applied extractors.
#[derive(Debug, Default)]
pub struct ExtractorResult {
    pub links: HashSet<ExtractedLink>,
    pub applied_extractors: HashSet<ExtractorMethod>,
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
#[derive(
    Debug,
    Copy,
    Clone,
    Default,
    Serialize,
    Deserialize,
    EnumString,
    Display,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
)]
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

impl Display for ExtractorCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.extractor_method, self.apply_when)
    }
}

impl ExtractorCommand {
    pub fn new(extractor_method: ExtractorMethod, apply_when: ApplyWhen) -> Self {
        Self {
            extractor_method,
            apply_when,
        }
    }

    pub fn new_default_apply(extractor_method: ExtractorMethod) -> Self {
        match &extractor_method {
            ExtractorMethod::BinaryHeuristic => Self::new(extractor_method, ApplyWhen::Fallback),
            _ => Self::new(extractor_method, Default::default()),
        }
    }

    pub fn can_apply(&self, page: &ProcessedData<'_>) -> bool {
        match self.apply_when {
            ApplyWhen::Always => true,
            ApplyWhen::IfSuitable => self.extractor_method.is_compatible(page),
            ApplyWhen::Fallback => false,
        }
    }

    pub fn is_fallback(&self) -> bool {
        return self.apply_when == ApplyWhen::Fallback;
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
    use crate::config::CrawlConfig;
    use crate::data::process;
    use crate::data::RawData;
    use crate::extraction::extractor::Extractor;
    use crate::fetching::FetchedRequestData;
    use crate::fetching::ResponseData;
    use crate::format::{AtraFileInformation, FileFormatData};
    use crate::test_impls::TestContext;
    use crate::toolkit::LanguageInformation;
    use crate::url::UrlWithDepth;

    #[tokio::test]
    async fn can_extract_data() {
        let page = ResponseData::from_response(
            FetchedRequestData::new(
                RawData::from_vec(include_bytes!("../../testdata/samples/HTML attribute reference - HTML_ HyperText Markup Language _ MDN.html").to_vec()),
                None,
                reqwest::StatusCode::OK,
                None,
                None,
                false
            ),
            UrlWithDepth::from_url("https://www.example.com/").unwrap()
        );

        let context = TestContext::default();

        let identified_type = AtraFileInformation::determine(&context, (&page).into());

        let preprocessed = process(&context, &page, &identified_type).await.unwrap();

        let mut cfg: CrawlConfig = Default::default();
        cfg.respect_nofollow = true;
        cfg.crawl_embedded_data = true;

        let extracted = Extractor::default()
            .extract(
                &context,
                &page,
                &identified_type,
                &preprocessed,
                Some(&LanguageInformation::ENG),
            )
            .await
            .to_optional_links()
            .unwrap();

        println!("{}", extracted.len());

        for link in extracted {
            println!("{}", link);
        }
    }
}
