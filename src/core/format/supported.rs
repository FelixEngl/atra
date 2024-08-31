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

use std::cmp::max;
use std::io::Read;
use std::str::FromStr;
use mime::{Mime, Name};
use serde::{Deserialize, Serialize};
use crate::core::contexts::Context;
use crate::core::format::file_format_detection::DetectedFileFormat;
use crate::core::format::mime::{MimeType};
use crate::core::response::ResponseData;

/// The inferred processable, type for a complete page for this crawler
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AtraSupportedFileFormat {
    HTML,
    PDF,
    JavaScript,
    PlainText,
    JSON,
    XML,
    Decodeable,
    Unknown // todo: Add identifier for binary
}

fn check_file_ending(respone: &ResponseData, endings: &[&'static str]) -> bool {
    if let Some(scheme) = respone.url.url().path() {
        let scheme = scheme.to_lowercase();
        endings.into_iter().any(|it| scheme.ends_with(it))
    } else {
        false
    }
}

fn html_heuristic(to_check: &[u8]) -> bool {
    #[inline(always)]
    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|window| window == needle)
    }

    let doctype = find_subsequence(to_check, b"<!DOCTYPE html");
    if let Some(ref doctype) = doctype {
        if 0usize.eq(doctype) {
            return true
        }
    }
    let html_start = find_subsequence(to_check, b"<html");
    let html_end = find_subsequence(to_check, b"</html>");

    if let Some(end) = html_end {
        if let Some(start) = html_start {
            if start < end {
                true
            } else {
                false
            }
        } else {
            false
        }
    } else if let Some(start) = html_start {
        if start == 0 {
            true
        } else if let Some(doctype) = doctype {
            doctype < start
        } else {
            false
        }
    } else {
        false
    }
}

macro_rules! supports_method {
    ($(
        typ: $typ: ident
        mime: $mime: ident
        $(file_endings: $endings: expr)?
    )+) => {
        fn name_2_supported_file_format(name: Name<'_>) -> Option<AtraSupportedFileFormat>{
            match name {
                $(
                    mime::$mime => Some(AtraSupportedFileFormat::$typ),
                )+
                _ => None
            }
        }

        fn extension_2_supported_file_format(respone: &ResponseData) -> Option<AtraSupportedFileFormat>{
            $(
                $(
                    if check_file_ending(respone, &$endings) {
                        return Some(AtraSupportedFileFormat::$typ)
                    }
                )?
            )+
            return None
        }
    };
}

impl AtraSupportedFileFormat {

    pub fn supports_decoding(&self) -> bool {
        matches!(self, Self::HTML | Self::JavaScript | Self::PlainText | Self::JSON | Self::XML | Self::Decodeable)
    }

    supports_method! {
        typ: HTML
        mime: HTML
        file_endings: ["html", "xhtml", "htm"]

        typ: PDF
        mime: PDF
        file_endings: ["pdf"]

        typ: JavaScript
        mime: JAVASCRIPT
        file_endings: ["js"]

        typ: PlainText
        mime: PLAIN
        file_endings: ["txt"]

        typ: JSON
        mime: JSON
        file_endings: ["json"]

        typ: XML
        mime: XML
        file_endings: ["xml"]

        typ: Decodeable
        mime: CSS

        typ: Decodeable
        mime: CSV
    }


    /// Tries to guess the supported file type.
    pub fn guess(
        page: &ResponseData,
        mime: Option<&MimeType>,
        file_format: Option<&DetectedFileFormat>,
        context: &impl Context
    ) -> AtraSupportedFileFormat {
        let mut is_text = false;

        if let Some(mime) = mime.map(|value| value.names_iter()) {
            for mime_name in mime {
                if let Some(by_mime) = Self::name_2_supported_file_format(mime_name) {
                    return by_mime
                }
                if mime_name == mime::TEXT {
                    is_text = true;
                }
            }
        }

        if let Some(found) = Self::extension_2_supported_file_format(page) {
            return found
        }

        if let Some(file_format) = file_format {
            let mime = Mime::from_str(file_format.most_probable_file_format().media_type()).expect("The mimes in file_format are always valid!");
            if let Some(suffix) = mime.suffix() {
                if let Some(found) = Self::name_2_supported_file_format(suffix) {
                    return found
                }
            }

            if let Some(found) = Self::name_2_supported_file_format(mime.subtype()) {
                return found
            }

            if let Some(found) = Self::name_2_supported_file_format(mime.type_()) {
                return found
            }
        }

        fn guess_if_html<const HAYSTACK_SIZE: usize>(context: &impl Context, page: &ResponseData) -> bool {
            if let Ok(Some(mut reader)) = page.content.cursor(context) {
                let mut haystack = [0u8;HAYSTACK_SIZE];
                match reader.read(&mut haystack) {
                    Ok(_) => {
                        html_heuristic(&haystack)
                    }
                    Err(_) => {
                        false
                    }
                }
            } else {
                false
            }
        }

        if is_text {
            if guess_if_html::<512>(context, page) {
                Self::HTML
            } else {
                Self::Decodeable
            }
        } else if let Ok(Some(reader)) = page.content.cursor(context) {
            let mut result = Vec::new();
            if context.configs().system.max_file_size_in_memory < 512 {
                if guess_if_html::<512>(context, page) {
                    Self::HTML
                } else {
                    Self::Unknown
                }
            } else {
                match reader.take(max(context.configs().system.max_file_size_in_memory, 512)).read_to_end(&mut result) {
                    Ok(_) => {
                        if html_heuristic(&result){
                            Self::HTML
                        } else {
                            Self::Unknown
                        }
                    }
                    Err(_) => {
                        Self::Unknown
                    }
                }
            }
        } else {
            Self::Unknown
        }


    }
}