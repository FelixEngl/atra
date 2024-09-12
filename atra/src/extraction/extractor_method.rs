use crate::contexts::traits::{SupportsConfigs, SupportsGdbrRegistry};
use crate::data::{Decoded, RawVecData};
use crate::extraction::extractor::{ExtractorResult, ProcessedData};
use crate::extraction::extractor_method::LinkExtractionError::NotCompatible;
use crate::extraction::links::ExtractedLink;
use crate::extraction::marker::{
    ExtractorMethodHint, ExtractorMethodMeta, ExtractorMethodMetaFactory,
};
use crate::extraction::raw::extract_possible_urls;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::toolkit::utf8::RobustUtf8Reader;
use bytes::Buf;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LinkExtractionError {
    #[error("The file can not be stored in memory and the extractor does not support off-memory extraction!")]
    CanNotStoreInMemory,
    #[error("The data is not compatible!")]
    NotCompatible,
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Was able to extract {successes} links but failed with: {errors:?}")]
    ExtractionErrors {
        successes: usize,
        errors: Vec<LinkExtractionSubError>,
    },
}

#[derive(Debug, Error)]
pub enum LinkExtractionSubError {
    #[cfg(not(windows))]
    #[error(transparent)]
    Pdf(#[from] link_scraper::formats::pdf::PdfScrapingError),
    #[error(transparent)]
    Rtf(#[from] link_scraper::formats::rtf::RtfScrapingError),
    #[error(transparent)]
    Ooxml(#[from] link_scraper::formats::ooxml::OoxmlScrapingError),
    #[error(transparent)]
    Odf(#[from] link_scraper::formats::odf::OdfScrapingError),
    #[error(transparent)]
    Exif(#[from] link_scraper::formats::image::ImageScrapingError),
    #[error(transparent)]
    Xml(#[from] link_scraper::formats::xml::XmlScrapingError),
    #[error(transparent)]
    Svg(#[from] link_scraper::formats::xml::svg::SvgScrapingError),
    #[error(transparent)]
    Xlink(#[from] link_scraper::formats::xml::xlink::XLinkFormatError),
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
    PdfV1,
}

impl ExtractorMethod {
    pub async fn extract_links<C>(
        &self,
        context: &C,
        page: &ProcessedData<'_>,
        output: &mut ExtractorResult,
    ) -> Result<usize, LinkExtractionError>
    where
        C: SupportsConfigs + SupportsGdbrRegistry,
    {
        if self.is_compatible(page) {
            return Err(NotCompatible);
        }
        match self {
            ExtractorMethod::HtmlV1 => extract_links_hml(self, context, page, output).await,
            ExtractorMethod::JSV1 => extract_links_javascript(self, page, output).await,
            ExtractorMethod::PlainText => extract_links_plain_text(self, page, output).await,
            ExtractorMethod::RawV1 => extract_links_raw(self, page, output).await,
            ExtractorMethod::Rtf => extract_links_rtf(self, page, output).await,
            ExtractorMethod::Ooxml => extract_links_ooxml(self, page, output).await,
            ExtractorMethod::Odf => extract_links_odf(self, page, output).await,
            ExtractorMethod::Exif => extract_links_exif(self, page, output).await,
            ExtractorMethod::Xml => extract_links_xml(self, page, output).await,
            ExtractorMethod::Svg => extract_links_svg(self, page, output).await,
            ExtractorMethod::Xlink => extract_links_xlink(self, page, output).await,
            #[cfg(not(windows))]
            ExtractorMethod::PdfV1 => extract_links_pdf(self, page, output).await,
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
    pub fn is_compatible(&self, page: &ProcessedData<'_>) -> bool {
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
            ExtractorMethod::RawV1 => true,
        }
    }
}

async fn extract_links_hml<C>(
    extractor: &impl ExtractorMethodMetaFactory,
    context: &C,
    page: &ProcessedData<'_>,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError>
where
    C: SupportsConfigs + SupportsGdbrRegistry,
{
    match &page.2 {
        Decoded::InMemory { data: result, .. } => {
            match crate::extraction::html::extract_links(
                &page.0.url,
                result.as_str(),
                context,
                page.3,
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
                                page.0.url
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
    page: &ProcessedData<'_>,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    match &page.2 {
        Decoded::InMemory { data: result, .. } => {
            let mut ct = 0usize;
            for entry in crate::extraction::js::extract_links(result.as_str()) {
                match ExtractedLink::pack(&page.0.url, entry.as_str(), extractor.new_without_meta())
                {
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
    page: &ProcessedData<'_>,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    match &page.2 {
        Decoded::InMemory { data: result, .. } => {
            let mut finder = linkify::LinkFinder::new();
            finder.kinds(&[linkify::LinkKind::Url]);
            let mut ct = 0usize;
            for entry in finder.links(result.as_str()) {
                match ExtractedLink::pack(&page.0.url, entry.as_str(), extractor.new_without_meta())
                {
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
    page: &ProcessedData<'_>,
    output: &mut ExtractorResult,
) -> Result<usize, LinkExtractionError> {
    async fn execute<'a, R: Read>(
        extractor: &impl ExtractorMethodMetaFactory,
        reader: RobustUtf8Reader<'a, R>,
        page: &ProcessedData<'_>,
        output: &mut ExtractorResult,
    ) -> Result<usize, LinkExtractionError> {
        let mut ct = 0usize;
        for entry in extract_possible_urls(reader)? {
            match ExtractedLink::pack(&page.0.url, &entry.0, extractor.new_without_meta()) {
                Ok(link) => {
                    if output.register_link(link) {
                        ct += 1;
                    }
                    continue;
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

    match page.2 {
        Decoded::InMemory {
            data: in_memory, ..
        } => {
            execute(
                extractor,
                RobustUtf8Reader::new(Cursor::new(in_memory)),
                page,
                output,
            )
            .await
        }
        Decoded::OffMemory { reference, .. } => {
            execute(
                extractor,
                RobustUtf8Reader::new(BufReader::new(File::options().read(true).open(reference)?)),
                page,
                output,
            )
            .await
        }
        Decoded::None => match &page.0.content {
            RawVecData::None => Ok(0),
            RawVecData::InMemory { data, .. } => {
                execute(
                    extractor,
                    RobustUtf8Reader::new(data.reader()),
                    page,
                    output,
                )
                .await
            }
            RawVecData::ExternalFile { file } => {
                execute(
                    extractor,
                    RobustUtf8Reader::new(BufReader::new(File::options().read(true).open(file)?)),
                    page,
                    output,
                )
                .await
            }
        },
    }
}

macro_rules! create_extraction_fn {
    ($vis: vis $name: ident(raw, $n: literal, $($tt:tt)+)) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &page.0.content {
                RawVecData::InMemory { data } => {
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
                RawVecData::ExternalFile { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
                RawVecData::None => { Ok(0) }
            }
        }

    };

    ($vis: vis $name: ident(decoded, $n: literal, $($tt:tt)+)) => {
        $vis async fn $name(extractor: &impl ExtractorMethodMetaFactory, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> Result<usize, LinkExtractionError> {
            match &page.2 {
                Decoded::InMemory { data, .. } => {
                    match $($tt)+::scrape(data.as_bytes()) {
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
                Decoded::OffMemory { .. } => { Err(LinkExtractionError::CanNotStoreInMemory) }
                Decoded::None => { Ok(0) }
            }
        }

    };
}

create_extraction_fn!(extract_links_rtf(raw, "rtf", link_scraper::formats::rtf));
create_extraction_fn!(extract_links_ooxml(
    raw,
    "ooxml",
    link_scraper::formats::ooxml
));
create_extraction_fn!(extract_links_odf(raw, "odf", link_scraper::formats::odf));
create_extraction_fn!(extract_links_exif(
    raw,
    "exif",
    link_scraper::formats::image
));
create_extraction_fn!(extract_links_xml(
    decoded,
    "xml",
    link_scraper::formats::xml
));
create_extraction_fn!(extract_links_svg(
    decoded,
    "svg",
    link_scraper::formats::xml::svg
));
create_extraction_fn!(extract_links_xlink(
    decoded,
    "xlink",
    link_scraper::formats::xml::xlink
));
#[cfg(not(windows))]
create_extraction_fn!(extract_links_pdf(raw, "pdf", link_scraper::formats::pdf));
