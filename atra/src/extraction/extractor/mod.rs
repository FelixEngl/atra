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

mod apply_when;
mod command;
mod data_holder;
mod result;

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess, SupportsGdbrRegistry};
use crate::data::Decoded;
use crate::extraction::extractor_method::ExtractorMethod;
use crate::fetching::ResponseData;
use crate::format::AtraFileInformation;
use crate::toolkit::LanguageInformation;
pub use apply_when::*;
use camino::Utf8PathBuf;
pub use command::*;
pub(crate) use data_holder::*;
pub use result::*;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

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
    pub fn can_extract_anything(&self, file_info: &AtraFileInformation) -> bool {
        self.0.iter().any(|extr| extr.can_extract(file_info))
    }

    /// If the flag [`FALLBACK_MODE`] is set, it makes sure that either the used extractor is
    /// a fallback or a non fallback
    async fn apply_extractors<const FALLBACK_MODE: bool, C>(
        &self,
        context: &C,
        data: ExtractorData<'_>,
        nesting: usize,
        result: &mut ExtractorResult,
    ) where
        C: SupportsConfigs + SupportsGdbrRegistry + SupportsFileSystemAccess,
    {
        for extractor in &self.0 {
            // Require that both are either true or false
            if FALLBACK_MODE ^ extractor.is_fallback() {
                continue;
            }
            if extractor.can_apply(data.file_info) {
                if result.apply_extractor(extractor.extractor_method) {
                    match extractor
                        .extractor_method
                        .extract_links(context, &data, nesting, result)
                        .await
                    {
                        Ok(value) => {
                            log::debug!("Extracted {value} links with {extractor}.");
                        }
                        Err(err) => {
                            log::warn!(
                                "Failed {extractor} for {} :: {:?} {} with: {}",
                                data.url.url,
                                data.file_name,
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
                    "{extractor} is not compatible with {} :: {:?} {}!",
                    data.url.url,
                    data.file_name,
                    data.file_info
                )
            }
        }
    }

    pub async fn extract_from_response<C>(
        &self,
        context: &C,
        response: &ResponseData,
        identified_type: &AtraFileInformation,
        decoded: &Decoded<String, Utf8PathBuf>,
        lang: Option<&LanguageInformation>,
    ) -> ExtractorResult
    where
        C: SupportsConfigs + SupportsGdbrRegistry + SupportsFileSystemAccess,
    {
        let data = ExtractorData::new_from_response(response, identified_type, decoded, lang);
        self.extract(context, 0, data).await
    }

    /// Extracts the data this the set extractors
    pub async fn extract<C>(&self, context: &C, nesting: usize, data: ExtractorData<'_>) -> ExtractorResult
    where
        C: SupportsConfigs + SupportsGdbrRegistry + SupportsFileSystemAccess,
    {
        if let Some(max_depth) = context.configs().crawl.max_extraction_depth {
            if nesting > max_depth {
                log::debug!("Reached max depth for extracting data {nesting}/{max_depth} for {}::{:?} - {}",
                    data.url.url,
                    data.file_name,
                    data.file_info.format
                );
                return ExtractorResult::default()
            }
        }
        let mut result = ExtractorResult::default();
        log::trace!(
            "Extractor: {}::{:?} - {}",
            data.url.url,
            data.file_name,
            data.file_info.format,
        );
        self.apply_extractors::<false, _>(context, data, nesting, &mut result)
            .await;
        if result.no_extractor_applied() || result.is_empty() {
            if !result.no_extractor_applied() {
                log::debug!("Extractor: Unsupported type: {:?}", data.file_info.format);
            }
            self.apply_extractors::<true, _>(context, data, nesting, &mut result)
                .await;
        }
        result
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self(
            ExtractorMethod::iter()
                .map(|value| ExtractorCommand::new_default_apply(value))
                .collect(),
        )
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
    use crate::format::determine_format_for_response;
    use crate::test_impls::TestContext;
    use crate::toolkit::LanguageInformation;
    use crate::url::UrlWithDepth;

    #[tokio::test]
    async fn can_extract_data() {
        let mut page = ResponseData::from_response(
            FetchedRequestData::new(
                RawData::from_vec(include_bytes!("../../../testdata/samples/HTML attribute reference - HTML_ HyperText Markup Language _ MDN.html").to_vec()),
                None,
                reqwest::StatusCode::OK,
                None,
                None,
                false
            ),
            UrlWithDepth::from_url("https://www.example.com/").unwrap()
        );

        let context = TestContext::default();

        let identified_type = determine_format_for_response(&context, &mut page);

        let preprocessed = process(&context, &page, &identified_type).await.unwrap();

        let mut cfg: CrawlConfig = Default::default();
        cfg.respect_nofollow = true;
        cfg.crawl_embedded_data = true;

        let extracted = Extractor::default()
            .extract_from_response(
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
