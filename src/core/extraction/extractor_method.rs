use thiserror::Error;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use crate::core::extraction::marker::{ExtractorMethodHint, ExtractorMethodMeta, ExtractorMethodMetaFactory};
use crate::core::extraction::extractor::{ProcessedData, ExtractorResult};
use crate::core::extraction::raw::extract_possible_urls;
use crate::core::format::supported::AtraSupportedFileFormat;
use crate::core::VecDataHolder;
use crate::core::contexts::Context;
use crate::core::extraction::links::ExtractedLink;
use crate::core::decoding::DecodedData;
use crate::core::extraction::extractor_method::LinkExtractionError::NotCompatible;

#[derive(Debug, Error)]
pub enum LinkExtractionError {
    #[error("The file can not be stored in memory and the extractor does not support off-memory extraction!")]
    CanNotStoreInMemory,
    #[error("The data is not compatible!")]
    NotCompatible,
    #[error("Was able to extract {successes} links but failed with: {errors:?}")]
    ExtractionErrors {
        successes: usize,
        errors: Vec<LinkExtractionSubError>
    }
}

#[derive(Debug, Error)]
pub enum LinkExtractionSubError {
    // #[error(transparent)]
    // Pdf(#[from] link_scraper::formats::pdf::PdfScrapingError),
    // #[error(transparent)]
    // Rtf(#[from] link_scraper::formats::rtf::RtfScrapingError),
}

macro_rules! create_extractor_method {
    ($(
        $name: ident {
            $(alias: $($alias: literal)|+)?;
            extractor: fn $extractor: ident;
        }
    )+) => {
       #[derive(Sequence, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone)]
        pub enum ExtractorMethod {
            $(
                $($(#[serde(alias = $alias)])+)?
                $name,
            )+
        }

        impl ExtractorMethod {
            pub async fn extract_links(&self, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
                if self.is_compatible(context, page) {
                    return Err(NotCompatible)
                }
                match self {
                    $(
                        ExtractorMethod::$name => $extractor(self, context, page, output).await,
                    )+
                }
            }
        }
    };
}

impl ExtractorMethodMetaFactory for ExtractorMethod {
    fn new_without_meta(&self) -> ExtractorMethodHint {
        ExtractorMethodHint::new_without_meta(self.clone())
    }

    fn new_with_meta(&self, meta: ExtractorMethodMeta) -> ExtractorMethodHint {
        ExtractorMethodHint::new_with_meta(self.clone(), meta)
    }
}



create_extractor_method! {
    HtmlV1 {
        alias: "HTML_v1";
        extractor: fn extract_links_hml;
    }

    JSV1 {
        alias: "js_v1" | "JavaScript_v1" | "JS_v1";
        extractor: fn extract_links_javascript;
    }

    PlainText {
        alias: "PlainText_v1" | "PT_v1" | "Plain_v1";
        extractor: fn extract_links_plain_text;
    }

    RawV1 {
        alias: "RAW_v1";
        extractor: fn extract_links_raw;
    }

    // PdfV1 {
    //     alias: "pdf_v1";
    //     extractor: fn extract_links_pdf;
    // }
    //
    // RTF {
    //     alias: "rtf_v1";
    //     extractor: fn extract_links_rtf;
    // }
}

impl ExtractorMethod {
    pub fn is_compatible(&self, context: &impl Context, page: &ProcessedData<'_>) -> bool {
        match self {
            ExtractorMethod::HtmlV1 => {
                matches!(page.1.format, AtraSupportedFileFormat::HTML)
            }
            ExtractorMethod::JSV1 => {
                !context.configs().crawl().crawl_javascript || !matches!(page.1.format, AtraSupportedFileFormat::JavaScript)
            }
            ExtractorMethod::PlainText => {
                matches!(page.1.format, AtraSupportedFileFormat::PlainText)
            }
            ExtractorMethod::RawV1 => {
                true
            }
            // ExtractorMethod::PdfV1 => {
            //     matches!(page.1.format, AtraSupportedFileFormat::PDF)
            // }
            // ExtractorMethod::RTF => {
            //     matches!(page.1.format, AtraSupportedFileFormat::RTF)
            // }
        }
    }
}


async fn extract_links_hml(extractor: &impl ExtractorMethodMetaFactory, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
    match &page.2 {
        DecodedData::InMemory { result, .. } => {
            match crate::core::extraction::html::extract_links(
                &page.0.url,
                result.as_str(),
                context.configs().crawl().respect_nofollow,
                context.configs().crawl().crawl_embedded_data,
                context.configs().crawl().crawl_javascript,
                context.configs().crawl().crawl_onclick_by_heuristic,
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
                            log::trace!("Error parsing '{}'\n---START---\n{message}\n---END---\n", page.0.url)
                        }
                    }
                    let mut ct = 0usize;
                    let base_ref = base.as_ref();
                    for (origin, link) in extracted {
                        match ExtractedLink::pack(base_ref, &link, extractor.new_with_meta(ExtractorMethodMeta::Html(origin))) {
                            Ok(link) => {
                                if link.is_not(base_ref) {
                                    if output.register_link(link) {
                                        ct += 1;
                                    }
                                }
                            }
                            Err(error) => {
                                log::debug!("Was not able to parse link {} from html. Error: {}", link, error)
                            }
                        }
                    }
                    Ok(ct)
                }
            }
        }
        DecodedData::OffMemory { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
        DecodedData::None => { Ok(0) }
    }
}

async fn extract_links_javascript(extractor: &impl ExtractorMethodMetaFactory, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
    match &page.2 {
        DecodedData::InMemory { result, .. } => {
            let mut ct = 0usize;
            for entry in crate::core::extraction::js::extract_links(result.as_str()) {
                match ExtractedLink::pack(&page.0.url, entry.as_str(), extractor.new_without_meta()) {
                    Ok(link) => {
                        if output.register_link(link) {
                            ct += 1;
                        }
                    }
                    Err(error) => {
                        log::debug!("Was not able to parse {} from javascript. Error: {}", entry, error)
                    }
                }
            }
            Ok(ct)
        }
        DecodedData::OffMemory { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
        DecodedData::None => { Ok(0) }
    }
}

async fn extract_links_plain_text(extractor: &impl ExtractorMethodMetaFactory, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
    match &page.2 {
        DecodedData::InMemory { result, .. } => {
            let mut finder = linkify::LinkFinder::new();
            finder.kinds(&[linkify::LinkKind::Url]);

            let mut ct = 0usize;
            for entry in finder.links(result.as_str()) {
                match ExtractedLink::pack(&page.0.url, entry.as_str(), extractor.new_without_meta()) {
                    Ok(link) => {
                        if output.register_link(link) {
                            ct += 1;
                        }
                    }
                    Err(error) => {
                        log::debug!("Was not able to parse {:?} from plain text. Error: {}", entry, error)
                    }
                }
            }
            Ok(ct)
        }
        DecodedData::OffMemory { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
        DecodedData::None => { Ok(0) }
    }
}

async fn extract_links_raw(extractor: &impl ExtractorMethodMetaFactory, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
    if let Some(in_memory) = page.2.as_in_memory() {
        let mut ct = 0usize;
        for entry in extract_possible_urls(in_memory.as_bytes()) {
            if let Some(encoding) = page.2.encoding() {
                let encoded = &encoding.decode(entry).0;
                match ExtractedLink::pack(
                    &page.0.url,
                    &encoded,
                    extractor.new_without_meta()
                ) {
                    Ok(link) => {
                        if output.register_link(link) {
                            ct += 1;
                        }
                        continue
                    }
                    Err(error) => {
                        log::debug!("Was not able to parse {:?} from raw. Error: {}", entry, error)
                    }
                }
            }
            let encoded = String::from_utf8_lossy(entry);
            match ExtractedLink::pack(
                &page.0.url,
                &encoded,
                extractor.new_without_meta()
            ) {
                Ok(link) => {
                    if output.register_link(link) {
                        ct += 1;
                    }
                }
                Err(error) => {
                    log::debug!("Was not able to parse {:?} from javascript. Error: {}", entry, error)
                }
            }

        }
        Ok(ct)
    } else {
        Ok(0)
    }
}

macro_rules! create_extraction_fn {
    ($name: ident($n: literal, $($tt:tt)+)) => {
        async fn $name(extractor: &impl ExtractorMethodMetaFactory, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &page.0.content {
                VecDataHolder::InMemory { data } => {

                    match $($tt)+::scrape(&data) {
                        Ok(result) => {
                            let mut ct = 0;
                            for value in result {
                                match ExtractedLink::pack(&page.0.url, &value.url, extractor.new_without_meta()) {
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
                VecDataHolder::ExternalFile { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
                VecDataHolder::None => { Ok(0) }
            }
        }

    };
}

// create_extraction_fn!(extract_links_pdf("pdf", link_scraper::formats::pdf));
// create_extraction_fn!(extract_links_rtf("rtf", link_scraper::formats::rtf));

