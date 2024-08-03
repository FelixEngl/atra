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

use std::io::Read;
use serde::{Deserialize, Serialize};
use crate::core::contexts::Context;
use crate::core::mime::{DocumentType, IS_DECODEABLE, IS_HTML, IS_JS, IS_JSON, IS_PDF, IS_PLAINTEXT, IS_XML, MimeType};
use crate::core::response::ResponseData;

/// The inferred processable, type for a complete page for this crawler
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum PageType {
    HTML,
    PDF,
    JavaScript,
    PlainText,
    JSON,
    XML,
    Decodeable,
    Unknown // todo: Add identifier for binary
}

impl PageType {
    fn check_file_ending<const N: usize>(page: &ResponseData, endings: [&'static str; N]) -> bool {
        if let Some(scheme) = page.url.url().path() {
            let scheme = scheme.to_lowercase();
            endings.into_iter().any(|it| scheme.ends_with(it))
        } else {
            false
        }
    }

    fn check_all<const N1: usize, const N2: usize>(
        page: &ResponseData,
        mime: &MimeType,
        types: [DocumentType; N1],
        endings: [&'static str; N2],
    ) -> bool {
        mime.check_has_document_type(types) || Self::check_file_ending(page, endings)
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    fn html_heuristic(to_check: &[u8]) -> bool {
        let doctype = Self::find_subsequence(to_check, b"<!DOCTYPE html");
        if let Some(ref doctype) = doctype {
            if 0usize.eq(doctype) {
                return true
            }
        }
        let html_start = Self::find_subsequence(to_check, b"<html");
        let html_end = Self::find_subsequence(to_check, b"</html>");

        if let Some(ref start) = html_start {
            if let Some(ref doctype) = doctype {
                if start < doctype {
                    return false
                }
            }
        } else {
            return false
        }
        if let Some(end) = html_end {
            if let Some(start) = html_start {
                if start < end {
                    return true
                }  else {
                    return false
                }
            } else {
                return false
            }
        }

        return false
    }

    pub fn infer(page: &ResponseData, mime: &MimeType, context: &impl Context) -> PageType {
        if Self::check_all(page, mime, IS_HTML, ["html", "xhtml", "htm"]) {
            Self::HTML
        } else if Self::check_all(page, mime, IS_PDF, ["pdf"]) {
            Self::PDF
        } else if Self::check_all(page, mime, IS_JS, ["js"]) {
            Self::JavaScript
        } else if Self::check_all(page, mime, IS_PLAINTEXT, ["txt"]) {
            Self::PlainText
        } else if Self::check_all(page, mime, IS_JSON, ["json"]) {
            Self::JSON
        } else if Self::check_all(page, mime, IS_XML, ["xml"]) {
            Self::XML
        } else if mime.check_has_document_type(IS_DECODEABLE) {
            Self::Decodeable
        } else {
            if let Ok(Some(reader)) = page.content.cursor(context) {
                let mut result = Vec::new();
                match reader.take(context.configs().system.max_file_size_in_memory).read_to_end(&mut result) {
                    Ok(_) => {
                        if Self::html_heuristic(&result){
                            Self::HTML
                        } else {
                            Self::Unknown
                        }
                    }
                    Err(_) => {
                        Self::Unknown
                    }
                }
            } else {
                Self::Unknown
            }
        }
    }
}