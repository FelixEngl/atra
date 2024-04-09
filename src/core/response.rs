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

use std::slice::Iter;
use encoding_rs::Encoding;
use file_format::FileFormat;
use scraper::Html;
use crate::core::mime::{DocumentType, EncodingSupplier, IS_HTML, MimeType, TypedMime};
use crate::static_selectors;
use url::Url;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use smallvec::SmallVec;
use crate::core::contexts::Context;
use crate::core::{UrlWithDepth, VecDataHolder};
use crate::core::fetching::{FetchedRequestData};
use crate::core::file_format_inference::infer_by_content;
use crate::core::page_type::PageType;


/// The response for a request
#[derive(Debug)]
pub struct ResponseData {
    /// The bytes of the resource.
    pub content: VecDataHolder,
    /// The url of the page
    pub url: UrlWithDepth,
    /// The headers of the page request response.
    pub headers: Option<HeaderMap>,
    /// The status code of the page request.
    pub status_code: StatusCode,
    /// The final destination of the page if redirects were performed [Not implemented in the chrome feature].
    pub final_redirect_destination: Option<String>,
    #[cfg(feature = "chrome")]
    /// Page object for chrome. The page may be closed when accessing it on another thread from concurrency.
    chrome_page: Option<chromiumoxide::Page>,
}

impl ResponseData {
    #[cfg(not(feature = "chrome"))]
    pub fn reconstruct(
        content: VecDataHolder,
        url: UrlWithDepth,
        headers: Option<HeaderMap>,
        status_code: StatusCode,
        final_redirect_destination: Option<String>,
    ) -> Self {
        Self {
            content,
            url,
            headers,
            status_code,
            final_redirect_destination,
        }
    }

    #[cfg(feature = "chrome")]
    pub fn reconstruct(
        content: VecDataHolder,
        url: UrlWithDepth,
        headers: Option<HeaderMap>,
        status_code: StatusCode,
        chrome_page: Option<chromiumoxide::Page>
    ) -> Self {
        Self {
            content,
            url,
            headers,
            status_code,
            final_redirect_destination,
            chrome_page
        }
    }

    pub fn new(
        page_response: FetchedRequestData,
        url: UrlWithDepth,
    ) -> Self {
        Self {
            content: page_response.content,
            url,
            headers: page_response.headers,
            status_code: page_response.status_code,
            final_redirect_destination: page_response.final_url,
        }
    }

    /// Returns a reference to the dataholder
    pub fn content(&self) -> &VecDataHolder {
        &self.content
    }

    /// Returns the parsed url
    pub fn get_url_parsed(&self) -> &Url {
        return &self.url.url
    }

    /// Returns the url used after resolving all redirects
    #[allow(dead_code)] pub fn get_url_final(&self) -> Url {
        if let Some(ref found) = self.final_redirect_destination {
            Url::parse(found.as_str()).unwrap_or_else(|_| self.url.url.clone())
        } else {
            self.url.url.clone()
        }
    }
}

/// A age with the extracted mime types
pub struct ResponseDataWithMeta<'a> {
    pub data: &'a ResponseData,
    pub mimetype: MimeType,
    pub page_type: PageType,
    pub file_formats: FileFormatInfo
}

pub struct FileFormatInfo {
    pub mime: SmallVec<[FileFormat; 4]>,
    pub magic: Option<FileFormat>,
    /// Can be useful at a later date
    pub extension: Option<FileFormat>
}

impl<'a> ResponseDataWithMeta<'a> {

    /// Creates a page with some associated mime type informations
    /// and the supported page type
    pub fn create_from(page: &'a ResponseData, context: &impl Context) -> Self {
        static_selectors! {
            [
                META_CONTENT = "meta[http-equiv=\"Content-Type\"][content]"
            ]
        }

        fn parse_page_raw(page: &[u8]) -> MimeType {
            Html::parse_document(&String::from_utf8_lossy(page))
                .select(&META_CONTENT)
                .filter_map(|value| value.attr("content").map(|value|value.parse::<MimeType>().unwrap()))
                .flatten()
                .collect()
        }

        let mime =
            if let Some(result) = page.headers.as_ref().map(|value| MimeType::from(value)) {
                if result.check_has_document_type(IS_HTML) {
                    if let Some(dat) = page.content.as_in_memory() {
                        result.into_iter().chain(parse_page_raw(dat.as_slice())).collect()
                    } else {
                        log::debug!("Unable to parse the html because of its size: {:?}!", page.content);
                        MimeType::None
                    }
                } else {
                    result
                }
            } else {
                if let Some(dat) = page.content.as_in_memory() {
                    parse_page_raw(dat.as_slice())
                } else {
                    log::debug!("Unable to parse the html because of its size!");
                    MimeType::None
                }
            };

        let magic = if let Ok(Some(value)) = page.content.cursor(context) {
            Some(infer_by_content(value))
        } else {
            None
        };


        let mut file_format: FileFormatInfo = FileFormatInfo {
            mime: SmallVec::new(),
            extension: None,
            magic
        };

        for mim in mime.iter() {
            if let Some(infered) =  crate::core::file_format_inference::infer_by_mime(mim.1.essence_str()) {
                for inf in infered {
                    if !file_format.mime.contains(inf) {
                        file_format.mime.push(inf.clone())
                    }
                }
            }
        }


        let type_suggestion = PageType::infer(page, &mime, context);

        ResponseDataWithMeta {
            data: page,
            mimetype: mime,
            file_formats: file_format,
            page_type: type_suggestion
        }
    }

    /// returns the extracted types mimes of the page
    pub fn get_mime_type(&self) -> &MimeType {
        &self.mimetype
    }

    /// Returns the page
    pub fn get_page(&self) -> &ResponseData {
        self.data
    }

    /// Extracts all encodings from the associated mime types
    pub fn get_decoding(&self) -> SmallVec<[&'static Encoding; 8]> {
        self.mimetype
            .iter()
            .map(|value| value.get_encoding())
            .flatten()
            .collect()
    }

    delegate::delegate! {
        to self.mimetype {
            // pub fn map<F, R>(&self, f: F) -> Option<SmallVec<[R; 8]>> where F: Fn(&TypedMime) -> R;
            #[allow(dead_code)] pub fn check_if<F>(&self, check: F) -> Option<bool> where F: Fn(&TypedMime) -> bool;
            pub fn check_has_document_type<const N: usize>(&self, types: [DocumentType; N]) -> bool;
            #[allow(dead_code)] pub fn iter(&self) -> Iter<TypedMime>;
        }
    }
}

impl AsRef<ResponseData> for ResponseDataWithMeta<'_> {
    fn as_ref(&self) -> &ResponseData {
        return self.data
    }
}

impl AsRef<MimeType> for ResponseDataWithMeta<'_> {
    fn as_ref(&self) -> &MimeType {
        self.get_mime_type()
    }
}


