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

use std::io;
use std::io::Write;
use std::net::{AddrParseError, IpAddr};
use std::num::ParseIntError;
use std::ops::Deref;
use std::str::{FromStr, ParseBoolError, Utf8Error};

use encoding_rs::Encoding;
use itertools::Either;
use reqwest::IntoUrl;
#[cfg(feature = "with_serde")]
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};
use thiserror::Error;
use time::error::Format;
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;
use ubyte::ByteUnit;
use crate::core::url::atra_uri::AtraUri;
use crate::core::UrlWithDepth;
use crate::warc::media_type::{MediaType, parse_media_type};
use crate::warc::record_type::WarcRecordType;
use crate::warc::truncated_reason::TruncatedReason;

/// Represents a WARC header defined by the standard.
///
/// All headers are camel-case versions of the standard names, with the hyphens removed.
#[allow(missing_docs)]
#[derive(Clone, Debug, Hash, Eq, PartialEq, EnumString, AsRefStr, Display)]
#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "with_serde", serde(into = "String"))]
#[cfg_attr(feature = "with_serde", serde(from = "String"))]
pub enum WarcFieldName {
    #[strum(to_string = "content-length")] ContentLength,
    #[strum(to_string = "content-type")] ContentType,
    #[strum(to_string = "warc-block-digest")] BlockDigest,
    #[strum(to_string = "warc-concurrent-to")] ConcurrentTo,
    #[strum(to_string = "warc-date")] Date,
    #[strum(to_string = "warc-filename")] Filename,
    #[strum(to_string = "warc-identified-payload-type")] IdentifiedPayloadType,
    #[strum(to_string = "warc-ip-address")] IPAddress,
    #[strum(to_string = "warc-payload-digest")] PayloadDigest,
    #[strum(to_string = "warc-profile")] Profile,
    #[strum(to_string = "warc-record-id")] WarcRecordId,
    #[strum(to_string = "warc-refers-to")] RefersTo,
    #[strum(to_string = "warc-refers-to-date")] RefersToDate,
    #[strum(to_string = "warc-refers-to-target-uri")] RefersToTargetUri,
    #[strum(to_string = "warc-segment-number")] SegmentNumber,
    #[strum(to_string = "warc-segment-origin-id")] SegmentOriginID,
    #[strum(to_string = "warc-segment-total-length")] SegmentTotalLength,
    #[strum(to_string = "warc-target-uri")] TargetURI,
    #[strum(to_string = "warc-truncated")] Truncated,
    #[strum(to_string = "warc-type")] WarcType,
    #[strum(to_string = "warc-warcinfo-id")] WarcInfoID,
    #[strum(to_string = "xx--atra--content-encoding")] ContentEncoding,
    #[strum(to_string = "xx--atra--external-file")] ExternalBinFile,
    #[strum(to_string = "xx--atra--base64")] Base64Encoded,
    #[strum(to_string = "xx--atra--header-length")] HeaderLength,
    #[strum(default)] Unknown(String),
}





#[derive(Debug, Error)]
pub enum WarcFieldValueWriteToError {
    #[error(transparent)]
    IOError(#[from]io::Error),
    #[error(transparent)]
    Format(#[from] Format)
}

#[derive(Debug, Error)]
pub enum WarcFieldValueParseError {
    #[error("New line in the value detected, this is illegal!")]
    IllegalNewLine,
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error("The Encoding is unknown to atra!")]
    UnknownEncoding(Vec<u8>),
    #[error(transparent)]
    TimeNotParseable(#[from] time::error::Parse),
    #[error(transparent)]
    IntNotParseable(#[from] ParseIntError),
    #[error(transparent)]
    BoolNotParseable(#[from] ParseBoolError),
    #[error(transparent)]
    AddressNotParseable(#[from] AddrParseError),
    #[error("Failed to parse mimetype with {0}")]
    MediaTypeNotParseable(String)
}

/// The values supported in the warc map
#[derive(Debug, Clone)]
pub enum WarcFieldValue {
    General(GeneralFieldValue),
    UriLike(UriLikeFieldValue),
    WarcRecordType(WarcRecordType),
    Encoding(&'static Encoding),
    ContentType(MediaType),
    Date(OffsetDateTime),
    Number(u64),
    Bool(bool),
    TruncatedReason(TruncatedReason),
    IPAddress(IpAddr),
    /// A fallback value, when nothing else works
    #[allow(dead_code)]
    Raw(Vec<u8>)
}

impl WarcFieldValue {

    pub fn parse(header: &WarcFieldName, buf: &[u8]) -> Result<WarcFieldValue, WarcFieldValueParseError> {
        if buf.contains(&b'\n') {
            return Err(WarcFieldValueParseError::IllegalNewLine)
        }
        let result = match header {
            WarcFieldName::WarcRecordId
            | WarcFieldName::ConcurrentTo
            | WarcFieldName::RefersTo
            | WarcFieldName::RefersToTargetUri
            | WarcFieldName::TargetURI
            | WarcFieldName::WarcInfoID
            | WarcFieldName::Profile
            | WarcFieldName::SegmentOriginID => {
                // Use unsafe to protect from bad user data
                WarcFieldValue::UriLike(unsafe{ UriLikeFieldValue::from_buffer_unchecked(buf)})
            }

            WarcFieldName::Base64Encoded => {
                WarcFieldValue::Bool(bool::from_str(&std::str::from_utf8(buf)?.to_lowercase())?)
            }

            WarcFieldName::WarcType => {
                //WarcRecordType
                WarcFieldValue::WarcRecordType(WarcRecordType::from_str(std::str::from_utf8(buf)?).unwrap())
            }

            WarcFieldName::ContentEncoding => {
                //Encoding
                match Encoding::for_label(buf) {
                    None => {return Err(WarcFieldValueParseError::UnknownEncoding(buf.to_vec()))}
                    Some(value) => {
                        WarcFieldValue::Encoding(value)
                    }
                }
            }

            WarcFieldName::Date | WarcFieldName::RefersToDate => {
                // Date
                WarcFieldValue::Date(OffsetDateTime::parse(std::str::from_utf8(buf)?, &Iso8601::DEFAULT)?)
            }

            WarcFieldName::ContentLength | WarcFieldName::SegmentNumber | WarcFieldName::SegmentTotalLength | WarcFieldName::HeaderLength => {
                // Number
                WarcFieldValue::Number(u64::from_str(std::str::from_utf8(buf)?)?)
            }

            WarcFieldName::ContentType | WarcFieldName::IdentifiedPayloadType=> {
                // ContentType
                WarcFieldValue::ContentType(
                    parse_media_type::<true>(buf).map_err(|err| WarcFieldValueParseError::MediaTypeNotParseable(err.to_string()))?.1
                )
            }
            WarcFieldName::Truncated => {
                // TruncatedReason
                WarcFieldValue::TruncatedReason(
                    TruncatedReason::from_str(std::str::from_utf8(buf)?).unwrap()
                )
            }

            WarcFieldName::IPAddress => {
                // IPAddress
                WarcFieldValue::IPAddress(
                    IpAddr::from_str(std::str::from_utf8(buf)?)?
                )
            }

            WarcFieldName::BlockDigest | WarcFieldName::Filename | WarcFieldName::PayloadDigest | WarcFieldName::Unknown(_) | WarcFieldName::ExternalBinFile => {
                // General
                // Use unsafe to protect from bad user data
                WarcFieldValue::General(unsafe{ GeneralFieldValue::from_buffer_unchecked(buf)})
            }
        };
        Ok(result)
    }

    pub fn write_to(&self, out: &mut impl Write) -> Result<usize, WarcFieldValueWriteToError> {
        Ok(
            match self {
                WarcFieldValue::General(value) => {
                    out.write(value.as_ref())?
                }
                WarcFieldValue::UriLike(value) => {
                    out.write(value.as_ref())?
                }
                WarcFieldValue::WarcRecordType(value) => {
                    out.write(value.as_ref().as_bytes())?
                }
                WarcFieldValue::ContentType(value) => {
                    out.write(value.to_string().as_bytes())?
                }
                WarcFieldValue::Date(value) => {
                    value.format_into(out, &Iso8601::DEFAULT)?
                }
                WarcFieldValue::Number(value) => {
                    out.write(value.to_string().as_bytes())?
                }
                WarcFieldValue::TruncatedReason(value) => {
                    out.write(value.to_string().as_bytes())?
                }
                WarcFieldValue::IPAddress(value) => {
                    out.write(value.to_string().as_bytes())?
                }
                WarcFieldValue::Raw(value) => {
                    out.write(value.as_ref())?
                }
                WarcFieldValue::Encoding(value) => {
                    out.write(value.name().as_bytes())?
                }
                WarcFieldValue::Bool(value) => {
                    out.write(if *value { b"true" } else { b"false" })?
                }
            }
        )
    }
}

impl From<GeneralFieldValue> for WarcFieldValue {
    #[inline] fn from(value: GeneralFieldValue) -> Self {
        Self::General(value)
    }
}

impl From<UriLikeFieldValue> for WarcFieldValue {
    #[inline] fn from(value: UriLikeFieldValue) -> Self {
        Self::UriLike(value)
    }
}

impl From<Either<ByteUnit, u64>> for WarcFieldValue {
    fn from(value: Either<ByteUnit, u64>) -> Self {
        match value {
            Either::Left(value) => {
                Self::Number(value.as_u64())
            }
            Either::Right(value) => {
                Self::Number(value)
            }
        }
    }
}


#[derive(Debug, Error)]
#[error("Newlines are not allowed in the header values!")]
pub struct IllegalNewlineError(pub Either<String, Vec<u8>>);

#[derive(Debug, Error)]
pub enum NotAnUriError {
    #[error("There is no scheme (see rfc3986) in this value, but this is the minimum requirement for identifying as an uri!")]
    SchemeMissing(GeneralFieldValue),
    #[error(transparent)]
    NewlineDetected(#[from] IllegalNewlineError)
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct GeneralFieldValue {
    value: Either<String, Vec<u8>>
}

impl GeneralFieldValue {
    #[allow(dead_code)]
    #[inline]
    pub fn from_string(value: impl Into<String>) -> Result<GeneralFieldValue, IllegalNewlineError> {
        Self::new(Either::Left(value.into()))
    }
    #[allow(dead_code)]
    #[inline]
    pub fn from_vec(value: Vec<u8>) -> Result<GeneralFieldValue, IllegalNewlineError> {
        Self::new(Either::Right(value))
    }

    #[allow(dead_code)]
    pub fn from_buffer(buf: &[u8]) -> Result<GeneralFieldValue, IllegalNewlineError> {
        match std::str::from_utf8(buf) {
            Ok(value) => {
                Self::from_string(value)
            }
            Err(_) => {
                Self::from_vec(buf.to_vec())
            }
        }
    }

    pub fn new(value: Either<String, Vec<u8>>) -> Result<GeneralFieldValue, IllegalNewlineError> {
        if Self::either_contains(&value, b"\n") {
            return Err(IllegalNewlineError(value))
        }
        Ok(Self{value})
    }

    #[allow(dead_code)] #[inline] pub unsafe fn from_string_unchecked(value: impl Into<String>) -> GeneralFieldValue {
        Self::new_unchecked(Either::Left(value.into()))
    }

    #[allow(dead_code)] #[inline] pub unsafe fn from_vec_unchecked(value: Vec<u8>) -> GeneralFieldValue {
        Self::new_unchecked(Either::Right(value))
    }

    pub unsafe fn from_buffer_unchecked(buf: &[u8]) -> GeneralFieldValue {
        Self::new_unchecked(match std::str::from_utf8(buf) {
            Ok(value) => {
                Either::Left(value.to_string())
            }
            Err(_) => {
                Either::Right(buf.to_vec())
            }
        })
    }

    pub const unsafe fn new_unchecked(value: Either<String, Vec<u8>>) -> GeneralFieldValue {
        Self{value}
    }

    fn either_get_bytes(target: &Either<String, Vec<u8>>) -> &[u8] {
        match target {
            Either::Left(value) => {value.as_bytes()}
            Either::Right(value) => {value.as_ref()}
        }
    }

    fn either_contains(target: &Either<String, Vec<u8>>, pattern: &[u8]) -> bool {
        assert!(!pattern.is_empty(), "Empty patterns are not allowed!");
        let b = Self::either_get_bytes(target);
        if pattern.len() == 1 {
            b.contains(&pattern[0])
        } else {
            memchr::memmem::find(b, pattern).is_some()
        }
    }

    /// Checks if the [pattern] is contained
    pub fn contains(
        &self,
        pattern: &[u8]
    ) -> bool {
        Self::either_contains(&self.value, pattern)
    }

    /// Returns true iff the value starts like a scheme (see rfc3986)
    pub fn starts_with_scheme(&self) -> bool {
        for c in self.as_ref() {
            let c = *c;
            if c == b':' {
                break
            }
            if c.is_ascii_alphanumeric() || c == b'-' || c == b'.' || c == b'_' || c == b'~' {
                continue
            }
            return false
        }
        return true
    }


    #[allow(dead_code)]
    pub fn into_inner(self) -> Either<String, Vec<u8>> {
        self.value
    }
}

impl AsRef<[u8]> for GeneralFieldValue {
    fn as_ref(&self) -> &[u8] {
        match &self.value {
            Either::Left(value) => {value.as_bytes()}
            Either::Right(value) => {value.as_ref()}
        }
    }
}

impl Deref for GeneralFieldValue {
    type Target = Either<String, Vec<u8>>;

    #[inline] fn deref(&self) -> &Self::Target {
        &self.value
    }
}


#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct UriLikeFieldValue {
    value: GeneralFieldValue
}

impl UriLikeFieldValue {
    #[allow(dead_code)]
    #[inline]
    pub fn from_string(value: impl Into<String>) -> Result<UriLikeFieldValue, NotAnUriError> {
        Self::new(GeneralFieldValue::from_string(value.into())?)
    }

    #[allow(dead_code)]
    #[inline]
    pub fn from_vec(value: Vec<u8>) -> Result<UriLikeFieldValue, NotAnUriError> {
        Self::new(GeneralFieldValue::from_vec(value)?)
    }

    #[allow(dead_code)]
    pub fn from_buffer(buf: &[u8]) -> Result<UriLikeFieldValue, NotAnUriError> {
        match std::str::from_utf8(buf) {
            Ok(value) => {
                Self::from_string(value.to_string())
            }
            Err(_) => {
                Self::from_vec(buf.to_vec())
            }
        }
    }

    pub fn new(value: GeneralFieldValue) -> Result<UriLikeFieldValue, NotAnUriError> {
        if !value.starts_with_scheme() {
            return Err(NotAnUriError::SchemeMissing(value))
        }
        Ok(unsafe{Self::new_unchecked(value)})
    }

    #[allow(dead_code)] #[inline] pub unsafe fn from_string_unchecked(value: impl Into<String>) -> UriLikeFieldValue {
        Self::new_unchecked(GeneralFieldValue::from_string_unchecked(value.into()))
    }

    #[allow(dead_code)] #[inline] pub unsafe fn from_vec_unchecked(value: Vec<u8>) -> UriLikeFieldValue {
        Self::new_unchecked(GeneralFieldValue::from_vec_unchecked(value))
    }


    pub unsafe fn from_buffer_unchecked(buf: &[u8]) -> UriLikeFieldValue {
        Self::new_unchecked(
            GeneralFieldValue::from_buffer_unchecked(
                buf
            )
        )
    }

    pub const unsafe fn new_unchecked(value: GeneralFieldValue) -> UriLikeFieldValue {
        Self{value}
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> GeneralFieldValue {
        self.value
    }
}

impl Deref for UriLikeFieldValue {
    type Target = GeneralFieldValue;

    #[inline] fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: IntoUrl> From<T> for UriLikeFieldValue {
    fn from(value: T) -> Self {
        UriLikeFieldValue::from_string(value.as_str().to_string()).expect("This does never fail!")
    }
}

impl From<AtraUri> for UriLikeFieldValue {
    fn from(value: AtraUri) -> Self {
        UriLikeFieldValue::from_string(value.to_string()).expect("This does never fail!")
    }
}

impl From<UrlWithDepth> for UriLikeFieldValue {
    fn from(value: UrlWithDepth) -> Self {
        UriLikeFieldValue::from(value.url)
    }
}