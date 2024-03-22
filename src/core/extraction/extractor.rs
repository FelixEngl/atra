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

use std::collections::HashSet;
use linkify::LinkKind;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use crate::core::contexts::Context;
use crate::core::decoding::DecodedData;
use crate::core::extraction::marker::{ExtractorMeta, ExtractorMetaFactory, SubExtractorMeta};
use crate::core::io::paths::DecodedDataFilePathBuf;
use super::ExtractedLink;
use crate::core::page_processing::{ProcessedPage};
use crate::core::page_type::PageType;
use crate::{declare_sub_extractor, define_decode_action};
use crate::core::extraction::raw::extract_possible_urls;

/*
    To register a new extractor, create a extractor_decode_action_declaration
    and extractor_sub_extractor_declaration.
*/

/// A struct acting as an extractor
#[derive(Debug, Serialize, Deserialize, Clone)]
#[repr(transparent)]
pub struct Extractor(pub Vec<SubExtractor>);

impl Extractor {
    /// Extracts the data this the set extractors
    pub async fn extract(&self, context: &impl Context, page: &ProcessedPage<'_>) -> ExtractorResult {
        log::trace!("Extractor: {:?} - {}", page.0.page_type, page.0.data.url);
        let mut result = ExtractorResult::default();
        for extractor in &self.0 {
            if let Some(extracted) = extractor.extract(page, context, result.links.len()).await {
                result.links.extend(extracted);
                result.applied_extractors.push(extractor.clone());
            }
        }
        if result.no_extractor_applied() {
            log::warn!("Extractor: Unsupported type: {:?}", page.0.page_type);
        }
        result
    }
}

impl Default for Extractor {
    fn default() -> Self {
        Self(SubExtractor::ALL_ENTRIES.to_vec())
    }
}

/// The result of an extraction, contains the extracted links as well es the applied extractors.
#[derive(Debug, Default)]
pub struct ExtractorResult {
    pub links: HashSet<ExtractedLink>,
    pub applied_extractors: Vec<SubExtractor>
}

impl ExtractorResult {
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



/// The decode action for a specifc sub extractor
pub(crate) trait DecodeAction {
    async fn decode(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, extracted_domains_count: usize) -> Option<HashSet<ExtractedLink>>;
    async fn decoded_small(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, data: &String) -> Option<HashSet<ExtractedLink>>;
    async fn decoded_big(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, path_to_file: &DecodedDataFilePathBuf) -> Option<HashSet<ExtractedLink>>;
    async fn not_decoded(page: &ProcessedPage<'_>, context: &impl Context) -> Option<HashSet<ExtractedLink>>;
}

declare_sub_extractor! {
    HtmlV1 | "HTML_v1" => HtmlV1Action;
    JavaScriptV1 | "JavaScript_v1" | "JS_v1" => JavaScriptV1Action;
    PlainTextV1 | "PlainText_v1" | "PT_v1" | "Plain_v1" => PlainTextV1Action;
    RawV1 | "RAW_v1" => RawV1Action;
}



define_decode_action! {
    struct HtmlV1Action: HTML;
    fn decoded(page, context, factory, result)-> {
        match crate::core::extraction::html::extract_links(
            &page.0.data.url,
            result.as_str(),
            context.configs().crawl().respect_nofollow,
            context.configs().crawl().crawl_embedded_data,
            context.configs().crawl().crawl_javascript,
            context.configs().crawl().crawl_onclick_by_heuristic,
        ) {
            None => { None }
            Some((base, extracted, errors)) => {
                if !errors.is_empty() {
                    if log::max_level() <= LevelFilter::Trace {
                        let mut message = String::new();
                        for err in errors {
                            message.push_str(err.as_ref());
                            message.push('\n');
                        }
                        log::trace!("Error parsing '{}'\n---START---\n{message}\n---END---\n", page.0.data.url)
                    }
                }
                let mut result = HashSet::new();
                let base_ref = base.as_ref();
                for (origin, link) in extracted {
                    match ExtractedLink::pack(base_ref, &link, factory.create_meta(SubExtractorMeta::Html(origin))) {
                        Ok(link) => {
                            if link.is_not(base_ref) {
                                result.insert(link);
                            }
                        }
                        Err(error) => {
                            log::warn!("Was not able to parse link {} from html. Error: {}", link, error)
                        }
                    }
                }
                Some(result)
            }
        }
    }
    fn decoded_file(_page, _context, _factory, _file) -> {
        log::warn!("Currently extracting links from big HTML files is not supported!");
        None
    }
}

define_decode_action! {
    struct JavaScriptV1Action: JavaScript;
    fn decoded(page, context, factory, data)-> {
        if !context.configs().crawl().crawl_javascript {
            None
        } else {
            let mut result = HashSet::new();
            for entry in crate::core::extraction::js::extract_links(data.as_str()) {
                match ExtractedLink::pack(&page.0.get_page().url, entry.as_str(), factory.create_empty_meta()) {
                    Ok(url) => {
                        result.insert(url);
                    }
                    Err(error) => {
                        log::error!("Was not able to parse {} from javascript. Error: {}", entry, error)
                    }
                }
            }
            Some(result)
        }
    }
    fn decoded_file(_page, _context, _factory, _file) -> {
        log::warn!("Currently extracting links from big JS files is not supported!");
        None
    }
}

define_decode_action! {
    struct PlainTextV1Action: PlainText;
    fn decoded(page, _context, factory, data)-> {
        let mut result = HashSet::new();
        let mut finder = linkify::LinkFinder::new();
        finder.kinds(&[LinkKind::Url]);
        for entry in finder.links(data.as_str()) {
            match ExtractedLink::pack(&page.0.get_page().url, entry.as_str(), factory.create_empty_meta()) {
                Ok(url) => {
                    result.insert(url);
                }
                Err(error) => {
                    log::warn!("Was not able to parse {} from plaintext. Error: {}", entry.as_str(), error)
                }
            }
        }
        Some(result)
    }
    fn decoded_file(_page, _context, _factory, _file) -> {
        log::warn!("Currently extracting links from big Text files is not supported!");
        None
    }
}

define_decode_action! {
    struct RawV1Action: fallback;
    fn decoded(page, _context, factory, _data)-> {
        let mut result = HashSet::new();
        if let Some(in_memory) = page.0.data.content.as_in_memory() {
            for entry in extract_possible_urls(in_memory.as_slice()) {
                if let Some(encoding) = page.1.encoding() {
                    let encoded = &encoding.decode(entry).0;
                    match ExtractedLink::pack(
                        &page.0.get_page().url,
                        &encoded,
                        factory.create_empty_meta()
                    ) {
                        Ok(link) => {
                            result.insert(link);
                            continue;
                        }
                        Err(err) => {
                            log::debug!("Was not able to parse {} from raw. Error: {}", encoded, err)
                        }
                    }
                }
                let encoded = String::from_utf8_lossy(entry);
                match ExtractedLink::pack(
                    &page.0.get_page().url,
                    &encoded,
                    factory.create_empty_meta()
                ) {
                    Ok(link) => {
                        result.insert(link);
                        continue;
                    }
                    Err(err) => {
                        log::debug!("Was not able to parse {} from raw. Error: {}", encoded, err)
                    }
                }

            }
        }
        Some(result)
    }
    fn decoded_file(_page, _context, _factory, _file) -> {
        log::warn!("Currently extracting links from big Text files is not supported!");
        None
    }
}




#[cfg(test)]
mod test {
    use crate::core::config::CrawlConfig;
    use crate::core::response::{ResponseData};
    use crate::core::page_processing::process;
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

