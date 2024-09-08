use thiserror::Error;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use crate::core::extraction::marker::{ExtractorMethodHint, ExtractorMethodMeta, ExtractorMethodMetaFactory};
use crate::core::extraction::extractor::{ProcessedData, ExtractorResult};
use crate::core::extraction::raw::extract_possible_urls;
use crate::core::format::supported::InterpretedProcessibleFileFormat;
use crate::core::contexts::Context;
use crate::core::extraction::links::ExtractedLink;
use crate::core::decoding::DecodedData;
use crate::core::extraction::extractor_method::LinkExtractionError::NotCompatible;
use crate::core::VecDataHolder;

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
    #[cfg(not(windows))] #[error(transparent)] Pdf(#[from] link_scraper::formats::pdf::PdfScrapingError),
    #[error(transparent)] Rtf(#[from] link_scraper::formats::rtf::RtfScrapingError),
    #[error(transparent)] Ooxml(#[from] link_scraper::formats::ooxml::OoxmlScrapingError),
    #[error(transparent)] Odf(#[from] link_scraper::formats::odf::OdfScrapingError),
    #[error(transparent)] Exif(#[from] link_scraper::formats::image::ImageScrapingError),
    #[error(transparent)] Xml(#[from] link_scraper::formats::xml::XmlScrapingError),
    #[error(transparent)] Svg(#[from] link_scraper::formats::xml::svg::SvgScrapingError),
    #[error(transparent)] Xlink(#[from] link_scraper::formats::xml::xlink::XLinkFormatError),
}

#[derive(Sequence, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone)]
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
    #[serde(alias = "RAW_v1")]
    RawV1,
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
    #[cfg(not(windows))]
    #[serde(alias = "pdf_v1")]
    PdfV1
}

impl ExtractorMethod {
    pub async fn extract_links(&self, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
        if self.is_compatible(context, page) {
            return Err(NotCompatible);
        }
        match self {
            ExtractorMethod::HtmlV1 => extract_links_hml(self, context, page, output).await,
            ExtractorMethod::JSV1 => extract_links_javascript(self, context, page, output).await,
            ExtractorMethod::PlainText => extract_links_plain_text(self, context, page, output).await,
            ExtractorMethod::RawV1 => extract_links_raw(self, context, page, output).await,
            ExtractorMethod::Rtf => extract_links_rtf(self, context, page, output).await,
            ExtractorMethod::Ooxml => extract_links_ooxml(self, context, page, output).await,
            ExtractorMethod::Odf => extract_links_odf(self, context, page, output).await,
            ExtractorMethod::Exif => extract_links_exif(self, context, page, output).await,
            ExtractorMethod::Xml => extract_links_xml(self, context, page, output).await,
            ExtractorMethod::Svg => extract_links_svg(self, context, page, output).await,
            ExtractorMethod::Xlink => extract_links_xlink(self, context, page, output).await,
            #[cfg(not(windows))] ExtractorMethod::PdfV1 => extract_links_pdf(self, context, page, output).await,
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
    pub fn is_compatible(&self, context: &impl Context, page: &ProcessedData<'_>) -> bool {
        match self {
            ExtractorMethod::HtmlV1 => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::HTML)
            }
            ExtractorMethod::JSV1 => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::JavaScript)
            }
            ExtractorMethod::PlainText => {
                matches!(
                    page.1.format,
                    InterpretedProcessibleFileFormat::PlainText
                    | InterpretedProcessibleFileFormat::StructuredPlainText
                    | InterpretedProcessibleFileFormat::ProgrammingLanguage
                )
            }
            #[cfg(not(windows))]
            ExtractorMethod::PdfV1 => {
                matches!(page.1.format, AtraSupportedFileFormat::PDF)
            }
            ExtractorMethod::Rtf => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::RTF)
            }
            ExtractorMethod::Ooxml => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::OOXML)
            }
            ExtractorMethod::Odf => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::ODF)
            }
            ExtractorMethod::Exif => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::IMAGE)
            }
            ExtractorMethod::Xml => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::XML)
            }
            ExtractorMethod::Svg => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::SVG)
            }
            ExtractorMethod::Xlink => {
                matches!(page.1.format, InterpretedProcessibleFileFormat::XML)
            }
            ExtractorMethod::RawV1 => {
                true
            }
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

async fn extract_links_javascript(extractor: &impl ExtractorMethodMetaFactory, _: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
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

    match page.2 {
        DecodedData::InMemory { result: in_memory, .. } => {
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
        }
        DecodedData::OffMemory { .. } => Ok(0),
        DecodedData::None => Ok(0)
    }
}

macro_rules! create_extraction_fn {
    ($vis: vis $name: ident($n: literal, $($tt:tt)+)) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
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


create_extraction_fn!(extract_links_rtf("rtf", link_scraper::formats::rtf));
create_extraction_fn!(extract_links_ooxml("ooxml", link_scraper::formats::ooxml));
create_extraction_fn!(extract_links_odf("odf", link_scraper::formats::odf));
create_extraction_fn!(extract_links_exif("exif", link_scraper::formats::image));
create_extraction_fn!(extract_links_xml("xml", link_scraper::formats::xml));
create_extraction_fn!(extract_links_svg("svg", link_scraper::formats::xml::svg));
create_extraction_fn!(extract_links_xlink("xlink", link_scraper::formats::xml::xlink));
#[cfg(not(windows))] create_extraction_fn!(extract_links_pdf("pdf", link_scraper::formats::pdf));
