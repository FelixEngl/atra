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

use std::convert::Infallible;
use std::slice::Iter;
use std::str::FromStr;
use encoding_rs::{Encoding, UTF_8};
use itertools::Itertools;
use mime::{MimeIter, Name};
use mime::Mime as RawMime;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};
use crate::core::mime::DocumentType::{AnyText, CSS, CSV, HTML, JavaScript, JSON, PDF, PlainText, TSV, XHTML, XML};


/// The document mime type
#[derive(Debug, Clone)]
pub enum MimeType {
    /// Multiple mime types are associated
    Multi(SmallVec<[TypedMime; 2]>),
    /// A single mime type
    Single(TypedMime),
    /// No mimetype
    None
}

impl MimeType {

    // /// Returns a resultset with the processed values in this.
    // /// Returns None if there are no mime types.
    // pub fn map<F, R>(&self, f: F) -> Option<SmallVec<[R; 8]>> where F: Fn(&TypedMime) -> R {
    //     match self {
    //         MimeType::Multi(values) => Some(values.iter().map(f).collect()),
    //         MimeType::Single(value) => Some(smallvec![f(value)]),
    //         MimeType::None => None
    //     }
    // }

    /// Checks if [check] is true for any value [TypedMime] in this.
    /// Returns None if there is no value to check
    pub fn check_if<F>(&self, check: F) -> Option<bool> where F: Fn(&TypedMime) -> bool {
        match self {
            MimeType::Multi(values) => {Some(values.iter().any(check))}
            MimeType::Single(value) => {Some(check(value))}
            MimeType::None => {None}
        }
    }

    /// Checks if this contains any of the provided [types]
    pub fn check_has_document_type<const N: usize>(&self, types: [DocumentType; N]) -> bool {
        self.check_if(|value| types.contains(&value.0)).unwrap_or(false)
    }

    pub fn iter(&self) -> Iter<TypedMime> {
        match self {
            MimeType::Multi(values) => {
                values.iter()
            }
            MimeType::Single(value) => {
                std::slice::from_ref(value).iter()
            }
            MimeType::None => {
                Iter::default()
            }
        }
    }
}

impl From<HeaderMap> for MimeType {
    fn from(value: HeaderMap) -> Self {
        Self::from(&value)
    }
}

impl From<&HeaderMap> for MimeType {
    fn from(value: &HeaderMap) -> Self {
        if let Some(content_type_header_value) = value.get(reqwest::header::CONTENT_TYPE) {
            if let Ok(content_type_header_value) = content_type_header_value.to_str() {
                content_type_header_value.parse().unwrap()
            } else {
                MimeType::None
            }
        } else {
            MimeType::None
        }
    }
}

impl FromStr for MimeType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Infallible> {
        Ok(MimeIter::new(s).into())
    }
}

impl<'a> From<MimeIter<'a>> for MimeType {
    fn from(value: MimeIter<'a>) -> Self {
        value
            .map_ok(|value| TypedMime::from(value))
            .collect()
    }
}

impl FromIterator<TypedMime> for MimeType {
    fn from_iter<T: IntoIterator<Item=TypedMime>>(iter: T) -> Self {
        let collected: SmallVec<[TypedMime; 2]> = iter.into_iter().collect();
        match collected.len() {
            0 => MimeType::None,
            1 => MimeType::Single(collected.into_iter().exactly_one().unwrap()),
            _ => MimeType::Multi(collected)
        }
    }
}

impl FromIterator<Option<TypedMime>> for MimeType {
    fn from_iter<T: IntoIterator<Item=Option<TypedMime>>>(iter: T) -> Self {
        let collected: Option<SmallVec<[TypedMime; 2]>> = iter.into_iter().collect();
        if let Some(collected) = collected {
            match collected.len() {
                0 => MimeType::None,
                1 => MimeType::Single(collected.into_iter().exactly_one().unwrap()),
                _ => MimeType::Multi(collected)
            }
        } else {
            MimeType::None
        }
    }
}

impl<E> FromIterator<Result<TypedMime, E>> for MimeType {
    fn from_iter<T: IntoIterator<Item=Result<TypedMime, E>>>(iter: T) -> Self {
        let collected: Result<SmallVec<[TypedMime; 2]>, _> = iter.into_iter().collect();
        if let Ok(collected) = collected {
            match collected.len() {
                0 => MimeType::None,
                1 => MimeType::Single(collected.into_iter().exactly_one().unwrap()),
                _ => MimeType::Multi(collected)
            }
        } else {
            MimeType::None
        }
    }
}

impl AsRef<[TypedMime]> for MimeType {
    fn as_ref(&self) -> &[TypedMime] {
        match self {
            MimeType::Multi(values) => &values,
            MimeType::Single(value) => std::array::from_ref(value),
            MimeType::None => &[]
        }
    }
}

impl IntoIterator for MimeType {
    type Item = <SmallVec<[TypedMime; 2]> as IntoIterator>::Item;
    type IntoIter = <SmallVec<[TypedMime; 2]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            MimeType::Multi(values) => {
                values.into_iter()
            }
            MimeType::Single(single) => {
                smallvec![single].into_iter()
            }
            MimeType::None => {
                SmallVec::new_const().into_iter()
            }
        }
    }
}


/// Supplies somehow an encoding.
pub trait EncodingSupplier {
    fn is_utf_8(&self) -> bool;

    fn get_encoding_name(&self) -> Option<Name>;

    fn get_encoding(&self) -> Option<&'static Encoding> {
        if self.is_utf_8() {
            Some(UTF_8)
        } else {
            self.get_encoding_name().map(|label| Encoding::for_label_no_replacement(label.as_str().as_bytes())).flatten()
        }
    }
}

impl EncodingSupplier for RawMime {
    fn is_utf_8(&self) -> bool {
        self.get_param(mime::UTF_8).is_some()
    }

    fn get_encoding_name(&self) -> Option<Name> {
        self.get_param(mime::CHARSET)
    }
}


/// A hard typing for some supported mime types. Usefull for identifying the correct type
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DocumentType {
    HTML,
    CSS,
    XHTML,
    PDF,
    CSV,
    TSV,
    JavaScript,
    PlainText,
    AnyText,
    Image,
    Audio,
    Video,
    DOCX,
    DOC,
    XLSX,
    XLS,
    PPTX,
    PPT,
    AnyApplication,
    XML,
    RichTextFormat,
    Font,
    JSON,
    /// Basically unknown, but the response is at least honest.
    OctetStream,
    Unknown
}

pub const IS_HTML:[DocumentType; 2] = [HTML, XHTML];
pub const IS_PDF:[DocumentType; 1] = [PDF];
pub const IS_JS:[DocumentType; 1] = [JavaScript];
pub const IS_PLAINTEXT:[DocumentType; 1] = [PlainText];
pub const IS_JSON:[DocumentType; 1] = [XML];
pub const IS_XML:[DocumentType; 1] = [JSON];

pub const IS_UTF8: [DocumentType; 2] = [XML, JSON];
pub const IS_DECODEABLE: [DocumentType; 8] = [HTML, XHTML, PlainText, JavaScript, CSS, CSV, TSV, AnyText];



/// A hard typing for some supported mime types. Usefull for identifying the correct type
#[derive(Clone, Debug)]
pub struct TypedMime(pub DocumentType, pub RawMime);

impl TypedMime {

    pub fn get_type(&self) -> Name {
        self.1.type_()
    }

    pub fn get_subtype(&self) -> Name {
        self.1.subtype()
    }
}

impl EncodingSupplier for TypedMime {
    fn is_utf_8(&self) -> bool {
        self.1.is_utf_8()
    }

    fn get_encoding_name(&self) -> Option<Name> {
        self.1.get_encoding_name()
    }
}

impl From<(RawMime, DocumentType)> for TypedMime {
    fn from(value: (RawMime, DocumentType)) -> Self {
        Self(value.1, value.0)
    }
}

impl From<(DocumentType, RawMime)> for TypedMime {
    fn from(value: (DocumentType, RawMime)) -> Self {
        Self(value.0, value.1)
    }
}

impl From<TypedMime> for (DocumentType, RawMime) {
    fn from(value: TypedMime) -> Self {
        (value.0, value.1)
    }
}

impl From<RawMime> for TypedMime {
    fn from(value: RawMime) -> Self {
        const DOCX_IDENT: &'static str = "vnd.openxmlformats-officedocument.wordprocessingml.document";
        const XLSX_IDENT: &'static str = "vnd.openxmlformats-officedocument.spreadsheetml.sheet";
        const PPTX_IDENT: &'static str = "vnd.openxmlformats-officedocument.presentationml.presentation";

        // For ne ones look here: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/MIME_types/Common_types
        let document_type = match (value.type_(), value.subtype()) {
            // Special types, that are always recognized, even if wringly declared main type.
            (_, mime::HTML)                         => DocumentType::HTML,
            (_, mime::XML)                          => DocumentType::XML,
            (_, mime::JSON)                         => DocumentType::JSON,
            (_, mime::JAVASCRIPT)                   => DocumentType::JavaScript,

            (mime::TEXT, mime::CSS)                 => DocumentType::CSS,
            (mime::TEXT, mime::PLAIN)               => DocumentType::PlainText,
            (mime::TEXT, mime::CSV)                 => DocumentType::CSV,
            (mime::TEXT, any) =>
                match any.as_str() {
                    "tab-separated-values"          => DocumentType::TSV,
                    _                               => DocumentType::AnyText
                }

            (mime::IMAGE, _)                        => DocumentType::Image,

            (mime::AUDIO, _)                        => DocumentType::Audio,

            (mime::VIDEO, _)                        => DocumentType::Video,

            (mime::APPLICATION, mime::PDF)          => DocumentType::PDF,
            (mime::APPLICATION, mime::OCTET_STREAM) => DocumentType::OctetStream,
            (mime::APPLICATION, any) =>
                match any.as_str() {
                    "x-httpd-php"                   => DocumentType::RichTextFormat,
                    "rdf"                           => DocumentType::RichTextFormat,
                    "xhtml"                         => DocumentType::XHTML,
                    "msword"                        => DocumentType::DOC,
                    DOCX_IDENT                      => DocumentType::DOCX,
                    "vnd.ms-excel"                  => DocumentType::XLS,
                    XLSX_IDENT                      => DocumentType::XLSX,
                    "PPTX_IDENT"                    => DocumentType::PPT,
                    PPTX_IDENT                      => DocumentType::PPTX,
                    _                               => DocumentType::AnyApplication,
                }

            (mime::FONT, _)                         => DocumentType::Font,

            // If nothing works it is unknown.
            _                                       => DocumentType::Unknown
        };

        Self(document_type, value)
    }
}


/// A typed mime collection supports various access methods
pub trait TypedMimeCollection {
    /// Checks if the mime collection has some kind of type
    fn is_of_type(&self, name: Name) -> bool;
    /// Checks if the mime collection has some kind of sub-type
    fn is_of_sub_type(&self, name: Name) -> bool;
    /// Collects all types from the underlying
    fn all_types(&self) -> Vec<Name>;
    fn all_sub_types(&self) -> Vec<Name>;
}

impl<T: AsRef<[TypedMime]>> TypedMimeCollection for T {

    fn is_of_type(&self, name: Name) -> bool {
        self.as_ref().iter().any(|it| it.get_type() == name)
    }

    fn is_of_sub_type(&self, name: Name) -> bool {
        self.as_ref().iter().any(|it| it.get_subtype() == name)
    }

    fn all_types(&self) -> Vec<Name> {
        self.as_ref().iter().map(|it| it.get_type()).unique().collect()
    }

    fn all_sub_types(&self) -> Vec<Name> {
        self.as_ref().iter().map(|it| it.get_subtype()).unique().collect()
    }

}
