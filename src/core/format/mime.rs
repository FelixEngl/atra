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
use strum::EnumIs;
use crate::core::format::mime_typing::MimeType;


/// The document mime type
#[derive(Debug, Clone, EnumIs, Serialize, Deserialize, Eq, PartialEq)]
pub enum MimeDescriptor {
    /// Multiple mime types are associated
    Multi(SmallVec<[TypedMime; 2]>),
    /// A single mime type
    Single(TypedMime),
    /// No mimetype
    Empty
}

impl MimeDescriptor {

    /// Checks if [check] is true for any value [TypedMime] in this.
    /// Returns None if there is no value to check
    pub fn check_if<F>(&self, check: F) -> Option<bool> where F: Fn(&TypedMime) -> bool {
        match self {
            MimeDescriptor::Multi(values) => {Some(values.iter().any(check))}
            MimeDescriptor::Single(value) => {Some(check(value))}
            MimeDescriptor::Empty => {None}
        }
    }

    /// Checks if this contains any of the provided [types]
    pub fn check_has_document_type<const N: usize>(&self, types: [MimeType; N]) -> bool {
        self.check_if(|value| types.contains(&value.0)).unwrap_or(false)
    }

    pub fn iter(&self) -> Iter<TypedMime> {
        match self {
            MimeDescriptor::Multi(values) => {
                values.iter()
            }
            MimeDescriptor::Single(value) => {
                std::slice::from_ref(value).iter()
            }
            MimeDescriptor::Empty => {
                Iter::default()
            }
        }
    }
}

impl From<HeaderMap> for MimeDescriptor {
    fn from(value: HeaderMap) -> Self {
        Self::from(&value)
    }
}

impl From<&HeaderMap> for MimeDescriptor {
    fn from(value: &HeaderMap) -> Self {
        if let Some(content_type_header_value) = value.get(reqwest::header::CONTENT_TYPE) {
            if let Ok(content_type_header_value) = content_type_header_value.to_str() {
                content_type_header_value.parse().unwrap()
            } else {
                MimeDescriptor::Empty
            }
        } else {
            MimeDescriptor::Empty
        }
    }
}

impl FromStr for MimeDescriptor {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Infallible> {
        Ok(MimeIter::new(s).into())
    }
}

impl<'a> From<MimeIter<'a>> for MimeDescriptor {
    fn from(value: MimeIter<'a>) -> Self {
        value
            .map_ok(|value| TypedMime::from(value))
            .collect()
    }
}

impl FromIterator<TypedMime> for MimeDescriptor {
    fn from_iter<T: IntoIterator<Item=TypedMime>>(iter: T) -> Self {
        let collected: SmallVec<[TypedMime; 2]> = iter.into_iter().collect();
        match collected.len() {
            0 => MimeDescriptor::Empty,
            1 => MimeDescriptor::Single(collected.into_iter().exactly_one().unwrap()),
            _ => MimeDescriptor::Multi(collected)
        }
    }
}

impl FromIterator<Option<TypedMime>> for MimeDescriptor {
    fn from_iter<T: IntoIterator<Item=Option<TypedMime>>>(iter: T) -> Self {
        let collected: Option<SmallVec<[TypedMime; 2]>> = iter.into_iter().collect();
        if let Some(collected) = collected {
            match collected.len() {
                0 => MimeDescriptor::Empty,
                1 => MimeDescriptor::Single(collected.into_iter().exactly_one().unwrap()),
                _ => MimeDescriptor::Multi(collected)
            }
        } else {
            MimeDescriptor::Empty
        }
    }
}

impl<E> FromIterator<Result<TypedMime, E>> for MimeDescriptor {
    fn from_iter<T: IntoIterator<Item=Result<TypedMime, E>>>(iter: T) -> Self {
        let collected: Result<SmallVec<[TypedMime; 2]>, _> = iter.into_iter().collect();
        if let Ok(collected) = collected {
            match collected.len() {
                0 => MimeDescriptor::Empty,
                1 => MimeDescriptor::Single(collected.into_iter().exactly_one().unwrap()),
                _ => MimeDescriptor::Multi(collected)
            }
        } else {
            MimeDescriptor::Empty
        }
    }
}

impl AsRef<[TypedMime]> for MimeDescriptor {
    fn as_ref(&self) -> &[TypedMime] {
        match self {
            MimeDescriptor::Multi(values) => &values,
            MimeDescriptor::Single(value) => std::array::from_ref(value),
            MimeDescriptor::Empty => &[]
        }
    }
}

impl IntoIterator for MimeDescriptor {
    type Item = <SmallVec<[TypedMime; 2]> as IntoIterator>::Item;
    type IntoIter = <SmallVec<[TypedMime; 2]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            MimeDescriptor::Multi(values) => {
                values.into_iter()
            }
            MimeDescriptor::Single(single) => {
                smallvec![single].into_iter()
            }
            MimeDescriptor::Empty => {
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
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct TypedMime(
    pub MimeType,
    #[serde(with = "MimeDef")]
    pub RawMime
);

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

impl From<(RawMime, MimeType)> for TypedMime {
    fn from(value: (RawMime, MimeType)) -> Self {
        Self(value.1, value.0)
    }
}

impl From<(MimeType, RawMime)> for TypedMime {
    fn from(value: (MimeType, RawMime)) -> Self {
        Self(value.0, value.1)
    }
}

impl From<TypedMime> for (MimeType, RawMime) {
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
            (_, mime::HTML)                         => MimeType::HTML,
            (_, mime::XML)                          => MimeType::XML,
            (_, mime::JSON)                         => MimeType::JSON,
            (_, mime::JAVASCRIPT)                   => MimeType::JavaScript,

            (mime::TEXT, mime::CSS)                 => MimeType::CSS,
            (mime::TEXT, mime::PLAIN)               => MimeType::PlainText,
            (mime::TEXT, mime::CSV)                 => MimeType::CSV,
            (mime::TEXT, any) =>
                match any.as_str() {
                    "tab-separated-values"          => MimeType::TSV,
                    _                               => MimeType::AnyText
                }

            (mime::IMAGE, _)                        => MimeType::Image,

            (mime::AUDIO, _)                        => MimeType::Audio,

            (mime::VIDEO, _)                        => MimeType::Video,

            (mime::APPLICATION, mime::PDF)          => MimeType::PDF,
            (mime::APPLICATION, mime::OCTET_STREAM) => MimeType::OctetStream,
            (mime::APPLICATION, any) =>
                match any.as_str() {
                    "x-httpd-php"                   => MimeType::RichTextFormat,
                    "rdf"                           => MimeType::RichTextFormat,
                    "xhtml"                         => MimeType::XHTML,
                    "msword"                        => MimeType::DOC,
                    DOCX_IDENT                      => MimeType::DOCX,
                    "vnd.ms-excel"                  => MimeType::XLS,
                    XLSX_IDENT                      => MimeType::XLSX,
                    "PPTX_IDENT"                    => MimeType::PPT,
                    PPTX_IDENT                      => MimeType::PPTX,
                    _                               => MimeType::AnyApplication,
                }

            (mime::FONT, _)                         => MimeType::Font,

            // If nothing works it is unknown.
            _                                       => MimeType::Unknown
        };

        Self(document_type, value)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(remote = "RawMime")]
struct MimeDef(
    #[serde(getter = "RawMime::to_string")]
    String
);

impl From<MimeDef> for RawMime {
    fn from(value: MimeDef) -> Self {
        RawMime::from_str(&value.0).unwrap()
    }
}

impl Into<MimeDef> for RawMime {
    fn into(self) -> MimeDef {
        MimeDef(self.to_string())
    }
}