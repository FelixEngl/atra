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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess, SupportsGdbrRegistry};
use crate::data::{Decoded, RawVecData};
use crate::extraction::deflate::extract_from_zip;
use crate::extraction::extractor::{ExtractorData, ExtractorResult};
use crate::extraction::links::ExtractedLink;
use crate::extraction::marker::{
    ExtractorMethodHint, ExtractorMethodMeta, ExtractorMethodMetaFactory,
};
use crate::extraction::raw::extract_possible_urls;
use crate::extraction::LinkExtractionError;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::AtraFileInformation;
use crate::toolkit::utf8::RobustUtf8Reader;
use bytes::Buf;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use strum::{Display, EnumCount, EnumIter};

#[derive(
    Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone, Display, EnumIter, EnumCount,
)]
pub enum ExtractorMethod {
    #[serde(alias = "HTML_v1")]
    HtmlV1,
    #[serde(alias = "js_v1")]
    #[serde(alias = "JavaScript_v1")]
    #[serde(alias = "JS_v1")]
    JSV1,
    #[serde(alias = "PlainText_v1")]
    #[serde(alias = "PT_v1")]
    #[serde(alias = "Plain_v1")]
    PlainText,
    #[serde(alias = "binary")]
    #[serde(alias = "heuristic")]
    #[serde(alias = "brute_force")]
    BinaryHeuristic,
    #[serde(alias = "rtf_v1")]
    Rtf,
    #[serde(alias = "ooxml")]
    Ooxml,
    #[serde(alias = "odf")]
    Odf,
    #[serde(alias = "exif")]
    #[serde(alias = "image")]
    Exif,
    #[serde(alias = "xml")]
    Xml,
    #[serde(alias = "svg")]
    Svg,
    #[serde(alias = "xlink")]
    Xlink,
    #[serde(alias = "zip")]
    Zip,
    #[cfg(not(windows))]
    #[serde(alias = "pdf_v1")]
    PdfV1,
}

impl ExtractorMethod {
    pub async fn extract_links<C>(
        &self,
        context: &C,
        page: &ExtractorData<'_>,
        nesting: usize,
        output: &mut ExtractorResult,
    ) -> Result<usize, LinkExtractionError>
    where
        C: SupportsConfigs + SupportsGdbrRegistry + SupportsFileSystemAccess,
    {
        if !self.is_compatible(page.file_info) {
            return Err(LinkExtractionError::NotCompatible);
        }
        match self {
            ExtractorMethod::BinaryHeuristic => Box::pin(extract_links_raw(self, page, nesting == 0, output)).await,
            ExtractorMethod::Zip => Box::pin(extract_links_zip(self, context, page, nesting, output)).await,
            ExtractorMethod::HtmlV1 => Box::pin(extract_links_html(self, context, page, nesting == 0, output)).await,
            ExtractorMethod::JSV1 => Box::pin(extract_links_javascript(self, page, nesting == 0, output)).await,
            ExtractorMethod::PlainText => Box::pin(extract_links_plain_text(self, page, nesting == 0, output)).await,
            ExtractorMethod::Rtf => Box::pin(extract_links_rtf(self, page, nesting == 0, output)).await,
            ExtractorMethod::Ooxml => Box::pin(extract_links_ooxml(self, page, nesting == 0, output)).await,
            ExtractorMethod::Odf => Box::pin(extract_links_odf(self, page, nesting == 0, output)).await,
            ExtractorMethod::Exif => Box::pin(extract_links_exif(self, page, nesting == 0, output)).await,
            ExtractorMethod::Xml => Box::pin(extract_links_xml(self, page, nesting == 0, output)).await,
            ExtractorMethod::Svg => Box::pin(extract_links_svg(self, page, nesting == 0, output)).await,
            ExtractorMethod::Xlink => Box::pin(extract_links_xlink(self, page, nesting == 0, output)).await,
            #[cfg(not(windows))]
            ExtractorMethod::PdfV1 => Box::pin(extract_links_pdf(self, page, nesting == 0, output)).await,
        }
    }
}

impl ExtractorMethodMetaFactory for ExtractorMethod {
    fn new_without_meta(&self) -> ExtractorMethodHint {
        ExtractorMethodHint::new_without_meta(self.clone())
    }

    fn new_with_meta(&self, meta: ExtractorMethodMeta) -> ExtractorMethodHint {
        ExtractorMethodHint::new_with_meta(self.clone(), meta)
    }
}

impl ExtractorMethod {
    pub fn is_compatible(&self, file_info: &AtraFileInformation) -> bool {
        match self {
            ExtractorMethod::HtmlV1 => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::HTML)
            }
            ExtractorMethod::JSV1 => {
                matches!(
                    file_info.format,
                    InterpretedProcessibleFileFormat::JavaScript
                )
            }
            ExtractorMethod::PlainText => {
                matches!(
                    file_info.format,
                    InterpretedProcessibleFileFormat::PlainText
                        | InterpretedProcessibleFileFormat::StructuredPlainText
                        | InterpretedProcessibleFileFormat::ProgrammingLanguage
                )
            }
            #[cfg(not(windows))]
            ExtractorMethod::PdfV1 => {
                matches!(file_info.format, AtraSupportedFileFormat::PDF)
            }
            ExtractorMethod::Rtf => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::RTF)
            }
            ExtractorMethod::Ooxml => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::OOXML)
            }
            ExtractorMethod::Odf => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::ODF)
            }
            ExtractorMethod::Exif => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::IMAGE)
            }
            ExtractorMethod::Xml => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::XML)
            }
            ExtractorMethod::Svg => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::SVG)
            }
            ExtractorMethod::Xlink => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::XML)
            }
            ExtractorMethod::Zip => {
                matches!(file_info.format, InterpretedProcessibleFileFormat::ZIP)
            }
            ExtractorMethod::BinaryHeuristic => {
                !matches!(file_info.format, InterpretedProcessibleFileFormat::ZIP)
            }
        }
    }
}

async fn extract_links_zip<C>(
    extractor: &impl ExtractorMethodMetaFactory,
    context: &C,
    data: &ExtractorData<'_>,
    nesting: usize,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError>
where
    C: SupportsGdbrRegistry + SupportsConfigs + SupportsFileSystemAccess,
{
    fn map_extracted_links(
        extractor: &impl ExtractorMethodMetaFactory,
        (mut name, result): (String, ExtractorResult),
        new: &mut ExtractorResult
    ) -> usize {
        name.shrink_to_fit();
        let mut ct = 0usize;
        for value in result.links {
            let success = match value {
                ExtractedLink::OnSeed {
                    extraction_method,
                    url,
                } => {
                    new.register_link(ExtractedLink::OnSeed {
                        url,
                        extraction_method: extractor.new_with_meta(ExtractorMethodMeta::Zip {
                            path: name.clone(),
                            underlying: Box::new(extraction_method),
                        }),
                    })
                }
                ExtractedLink::Outgoing {
                    extraction_method,
                    url,
                } => {
                    new.register_link(ExtractedLink::Outgoing {
                        url,
                        extraction_method: extractor.new_with_meta(ExtractorMethodMeta::Zip {
                            path: name.clone(),
                            underlying: Box::new(extraction_method),
                        }),
                    })
                }
                ExtractedLink::Data {
                    extraction_method,
                    url,
                    base,
                } => {
                    new.register_link(ExtractedLink::Data {
                        url,
                        base,
                        extraction_method: extractor.new_with_meta(ExtractorMethodMeta::Zip {
                            path: name.clone(),
                            underlying: Box::new(extraction_method),
                        }),
                    })
                }
            };
            if success {
                ct += 1
            }
        }
        ct
    }

    if let Some(value) = data.raw_data.cursor()? {
        match extract_from_zip(data.url, BufReader::new(value), nesting, context).await {
            Ok((result, errors)) => {
                if !errors.is_empty() {
                    if log::max_level() <= log::LevelFilter::Trace {
                        let mut message = String::new();
                        for (path, err) in errors {
                            message.push_str(&format!("Error at {path} in zip:\n"));
                            message.push_str(err.to_string().as_str());
                            message.push('\n');
                        }
                        log::trace!("Error parsing '{}'\n---START---\n{message}\n---END---\n",data.url)
                    }
                }

                let mut ct = 0usize;
                for link in result {
                    ct += map_extracted_links(
                        extractor,
                        link,
                        output
                    );
                }
                Ok(ct)
            }
            Err(err) => {
                log::debug!("Failed to extract from zip file {}:\n{err}", data.url.url);
                Ok(0)
            }
        }
    } else {
        Ok(0)
    }
}

async fn extract_links_html<C>(
    extractor: &impl ExtractorMethodMetaFactory,
    context: &C,
    data: &ExtractorData<'_>,
    use_base: bool,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError>
where
    C: SupportsConfigs + SupportsGdbrRegistry,
{
    match &data.decoded {
        Decoded::InMemory { data: result, .. } => {
            match crate::extraction::html::extract_links(
                &data.url,
                result.as_str(),
                context,
                data.language,
            ) {
                None => Ok(0),
                Some((base, extracted, errors)) => {
                    if !errors.is_empty() {
                        if log::max_level() <= log::LevelFilter::Trace {
                            let mut message = String::new();
                            for err in errors {
                                message.push_str(err.as_ref());
                                message.push('\n');
                            }
                            log::trace!(
                                "Error parsing '{}'\n---START---\n{message}\n---END---\n",
                                data.url
                            )
                        }
                    }
                    let mut ct = 0usize;
                    let base_ref = base.as_ref();
                    for (origin, link) in extracted {
                        match ExtractedLink::pack(
                            base_ref,
                            &link,
                            extractor.new_with_meta(ExtractorMethodMeta::Html(origin)),
                            use_base
                        ) {
                            Ok(link) => {
                                if link.is_not(base_ref) {
                                    if output.register_link(link) {
                                        ct += 1;
                                    }
                                }
                            }
                            Err(error) => {
                                log::debug!(
                                    "Was not able to parse link {} from html. Error: {}",
                                    link,
                                    error
                                )
                            }
                        }
                    }
                    Ok(ct)
                }
            }
        }
        Decoded::OffMemory { .. } => Err(LinkExtractionError::CanNotStoreInMemory),
        Decoded::None => Ok(0),
    }
}

async fn extract_links_javascript(
    extractor: &impl ExtractorMethodMetaFactory,
    data: &ExtractorData<'_>,
    use_base: bool,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    match &data.decoded {
        Decoded::InMemory { data: result, .. } => {
            let mut ct = 0usize;
            for entry in crate::extraction::js::extract_links(result.as_str()) {
                match ExtractedLink::pack(&data.url, entry.as_str(), extractor.new_without_meta(), use_base) {
                    Ok(link) => {
                        if output.register_link(link) {
                            ct += 1;
                        }
                    }
                    Err(error) => {
                        log::debug!(
                            "Was not able to parse {} from javascript. Error: {}",
                            entry,
                            error
                        )
                    }
                }
            }
            Ok(ct)
        }
        Decoded::OffMemory { .. } => Err(LinkExtractionError::CanNotStoreInMemory),
        Decoded::None => Ok(0),
    }
}

async fn extract_links_plain_text(
    extractor: &impl ExtractorMethodMetaFactory,
    data: &ExtractorData<'_>,
    use_base: bool,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    match &data.decoded {
        Decoded::InMemory { data: result, .. } => {
            let mut finder = linkify::LinkFinder::new();
            finder.kinds(&[linkify::LinkKind::Url]);
            let mut ct = 0usize;
            for entry in finder.links(result.as_str()) {
                match ExtractedLink::pack(&data.url, entry.as_str(), extractor.new_without_meta(), use_base) {
                    Ok(link) => {
                        if output.register_link(link) {
                            ct += 1;
                        }
                    }
                    Err(error) => {
                        log::debug!(
                            "Was not able to parse {:?} from plain text. Error: {}",
                            entry,
                            error
                        )
                    }
                }
            }
            Ok(ct)
        }
        Decoded::OffMemory { .. } => Err(LinkExtractionError::CanNotStoreInMemory),
        Decoded::None => Ok(0),
    }
}

async fn extract_links_raw(
    extractor: &impl ExtractorMethodMetaFactory,
    data: &ExtractorData<'_>,
    use_base: bool,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    async fn execute<'a, R: Read>(
        extractor: &impl ExtractorMethodMetaFactory,
        reader: RobustUtf8Reader<'a, R>,
        page: &ExtractorData<'_>,
        use_base: bool,
        output: &mut ExtractorResult,
    ) -> Result<usize, LinkExtractionError> {
        let mut ct = 0usize;
        for entry in extract_possible_urls(reader)? {
            match ExtractedLink::pack(&page.url, &entry.0, extractor.new_without_meta(), use_base) {
                Ok(link) => {
                    if output.register_link(link) {
                        ct += 1;
                    }
                }
                Err(error) => {
                    log::debug!(
                        "Was not able to parse {:?} from raw. Error: {}",
                        entry,
                        error
                    )
                }
            }
        }
        Ok(ct)
    }

    match data.decoded {
        Decoded::InMemory {
            data: in_memory, ..
        } => {
            execute(
                extractor,
                RobustUtf8Reader::new(Cursor::new(in_memory)),
                data,
                use_base,
                output,
            )
            .await
        }
        Decoded::OffMemory { reference, .. } => {
            execute(
                extractor,
                RobustUtf8Reader::new(BufReader::new(File::options().read(true).open(reference)?)),
                data,
                use_base,
                output,
            )
            .await
        }
        Decoded::None => match &data.raw_data {
            RawVecData::None => Ok(0),
            RawVecData::InMemory {
                data: in_memory_data,
                ..
            } => {
                execute(
                    extractor,
                    RobustUtf8Reader::new(in_memory_data.reader()),
                    data,
                    use_base,
                    output,
                )
                .await
            }
            RawVecData::ExternalFile { path } => {
                execute(
                    extractor,
                    RobustUtf8Reader::new(BufReader::new(File::options().read(true).open(path)?)),
                    data,
                    use_base,
                    output,
                )
                .await
            }
        },
    }
}

macro_rules! define_method {
    ($vis: vis $name: ident raw@(
        name: $n: literal
        in_memory: fn($in_var: ident) $in_block: block
        off_memory: fn($off_var: ident) $off_block: block
    )) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, data: &ExtractorData<'_>, use_base: bool, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &data.raw_data {
                RawVecData::InMemory { data: $in_var } => {
                    match $in_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                RawVecData::ExternalFile { path: $off_var } => {
                    match $off_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                RawVecData::None => { Ok(0) }
            }
        }
    };
    ($vis: vis $name: ident raw@(
        name: $n: literal
        in_memory: fn($in_var: ident) $in_block: block
    )) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, data: &ExtractorData<'_>, use_base: bool, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &data.raw_data {
                RawVecData::InMemory { data: $in_var } => {
                    match $in_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                RawVecData::ExternalFile { .. } => Err(LinkExtractionError::CanNotStoreInMemory),
                RawVecData::None => { Ok(0) }
            }
        }
    };
    ($vis: vis $name: ident decoded@(
        name: $n: literal
        in_memory: fn($in_var: ident) $in_block: block
        off_memory: fn($off_var: ident) $off_block: block
    )) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, data: &ExtractorData<'_>, use_base: bool, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &data.decoded {
                Decoded::InMemory { data: $in_var, .. } => {
                    match $in_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                Decoded::OffMemory { reference: $off_var, .. } => {
                    match $off_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                Decoded::None => { Ok(0) }
            }
        }
    };
    ($vis: vis $name: ident decoded@(
        name: $n: literal
        in_memory: fn($in_var: ident) $in_block: block
    )) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, data: &ExtractorData<'_>, use_base: bool, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &data.decoded {
                Decoded::InMemory { data: $in_var, .. } => {
                    match $in_block {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&data.url, &value.url, extractor.new_without_meta(), use_base) {
                                    Ok(link) => {
                                        if output.register_link(link) {
                                            ct += 1;
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse {:?} from {}. Error: {}", value, $n, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                        Err(err) => {
                            log::error!("Failed to scrape {}: {err:?}", $n);
                            Err(LinkExtractionError::ExtractionErrors {
                                errors: vec![err.into()],
                                successes: 0
                            })
                        }
                    }
                }
                Decoded::OffMemory { .. } => Err(LinkExtractionError::CanNotStoreInMemory),
                Decoded::None => { Ok(0) }
            }
        }
    };
}

define_method! {
    extract_links_xml raw@(
        name:"xml"
        in_memory: fn(data) {
            link_scraper::formats::xml::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::xml::scrape_from_file(path)
        }
    )
}

define_method! {
    extract_links_svg raw@(
        name:"svg"
        in_memory: fn(data) {
            link_scraper::formats::xml::svg::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::xml::svg::scrape_from_file(path)
        }
    )
}

define_method! {
    extract_links_rtf decoded@(
        name:"rtf"
        in_memory: fn(data) {
            link_scraper::formats::rtf::scrape_from_string(data)
        }
    )
}

define_method! {
    extract_links_ooxml raw@(
        name:"ooxml"
        in_memory: fn(data) {
            link_scraper::formats::ooxml::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::ooxml::scrape_from_file(path)
        }
    )
}

define_method! {
    extract_links_odf raw@(
        name:"odf"
        in_memory: fn(data) {
            link_scraper::formats::odf::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::odf::scrape_from_file(path)
        }
    )
}

define_method! {
    extract_links_exif raw@(
        name:"exif"
        in_memory: fn(data) {
            link_scraper::formats::image::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::image::scrape_from_file(path)
        }
    )
}

define_method! {
    extract_links_xlink raw@(
        name:"xlink"
        in_memory: fn(data) {
            link_scraper::formats::xml::xlink::scrape_from_slice(data)
        }
        off_memory: fn(path) {
            link_scraper::formats::xml::xlink::scrape_from_file(path)
        }
    )
}

#[cfg(not(windows))]
define_method! {
    extract_links_pdf raw@(
        name:"pdf"
        in_memory: fn(data) {
            link_scraper::formats::pdf::scrape_from_slice(data)
        }
    )
}