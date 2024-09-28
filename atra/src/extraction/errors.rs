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

use thiserror::Error;
use zip::result::ZipError;

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
    #[error(transparent)]
    ZipError(#[from] ZipError),
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
