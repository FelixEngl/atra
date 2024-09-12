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

// https://iipc.github.io/warc-specifications/specifications/warc-format/warc-1.1-annotated/

use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::io::Write;
use std::net::IpAddr;

use encoding_rs::Encoding;
use itertools::Itertools;
use paste::paste;
use thiserror::Error;

use crate::field::*;
use crate::media_type::MediaType;
use crate::record_type::WarcRecordType;
use crate::truncated_reason::TruncatedReason;

/// The supported warc version
pub const WARC_VERSION: &[u8] = b"WARC/1.1";

/// A simple warc record header
#[derive(Debug, Clone)]
pub struct WarcHeader {
    version: Option<String>,
    warc_headers: HashMap<WarcFieldName, WarcFieldValue>,
}

/// Errors returned when something wents wrong with the set method.
#[derive(Debug, Error)]
pub enum WarcHeaderValueError {
    #[error(transparent)]
    NewlineContained(#[from] IllegalNewlineError),
    #[error("The warc header {0} does not allow spaces!")]
    WhitespaceContained(WarcFieldName),
    #[error(transparent)]
    NotAnUri(#[from] NotAnUriError),
    #[error("Was not able to detect a ':' in the value!")]
    DigestIsMissingAlgorithm,
}

/// Errors encountered when writing a WarcHeader
#[derive(Debug, Error)]
pub enum WarcHeaderWriteError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("The mandatory values {0:?} are missing!")]
    MandatoryValuesAreMissing(Vec<WarcFieldName>),
    #[error(transparent)]
    ValueWriterError(#[from] WarcFieldValueWriteToError),
}

type WarcHeaderResult = Result<Option<WarcFieldValue>, WarcHeaderValueError>;

macro_rules! create_setter {
    ($target: ident with $name: ident($self: ident, $var_name: ident: $typ: ty) -> $value_typ: ident) => {

        pub fn $name(&mut $self, $var_name: $typ) -> WarcHeaderResult {
            unsafe {
                Ok($self.unchecked_field(
                    WarcFieldName::$target,
                    WarcFieldValue::$value_typ($var_name)
                ))
            }
        }
    };
    (general@$target: ident with $name: ident($self: ident)) => {

        pub fn $name(&mut $self, general: GeneralFieldValue) -> WarcHeaderResult {
            unsafe {
                Ok($self.unchecked_field(
                    WarcFieldName::$target,
                    general.into()
                ))
            }
        }
        paste::paste!{
             #[inline] pub fn [<$name _string>](&mut $self, general: &str) -> WarcHeaderResult {
                $self.$name(general.parse()?)
            }
             #[inline] pub fn [<$name _bytes>](&mut $self, general: Vec<u8>) -> WarcHeaderResult {
                $self.$name(GeneralFieldValue::from_vec(general)?)
            }
        }
    };

    (general@$target: ident with $name: ident($self: ident) and fn check($value: ident) $check_block: block) => {
         pub fn $name(&mut $self, general: GeneralFieldValue) -> WarcHeaderResult {
            fn check($value: GeneralFieldValue) -> Result<GeneralFieldValue, WarcHeaderValueError> $check_block
            unsafe {
                Ok($self.unchecked_field(
                    WarcFieldName::$target,
                    check(general)?.into()
                ))
            }
        }
        paste::paste!{
             #[inline] pub fn [<$name _string>](&mut $self, general: &str) -> WarcHeaderResult {
                $self.$name(general.parse()?)
            }
             #[inline] pub fn [<$name _bytes>](&mut $self, general: Vec<u8>) -> WarcHeaderResult {
                $self.$name(GeneralFieldValue::from_vec(general)?)
            }
        }
    };
    (uri@$target: ident with $name: ident($self: ident)  ) => {
         pub fn $name(&mut $self, uri_like: UriLikeFieldValue) -> WarcHeaderResult {
            if uri_like.contains(b" ") {
                return Err(WarcHeaderValueError::WhitespaceContained(WarcFieldName::$target))
            }
            unsafe {
                Ok($self.unchecked_field(
                    WarcFieldName::$target,
                    uri_like.into()
                ))
            }
        }
        paste::paste!{
             #[inline] pub fn [<$name _string>](&mut $self, general: &str) -> WarcHeaderResult {
                $self.$name(general.parse()?)
            }
             #[inline] pub fn [<$name _bytes>](&mut $self, general: Vec<u8>) -> WarcHeaderResult {
                $self.$name(UriLikeFieldValue::from_vec(general)?)
            }
        }
    };
}

#[derive(Debug, Error)]
pub enum RequiredFieldError<'a> {
    #[error("No value found for {0}!")]
    NotFound(WarcFieldName),
    #[error("Was {0} not the expected type but {1:?}.")]
    WrongType(WarcFieldName, &'a WarcFieldValue),
}

macro_rules! create_getter {
    (optional@$target: ident with $name: ident($self: ident) -> $value_ident: ident as $typ: ty) => {

        pub fn $name(&$self) -> Option<Result<&$typ, &WarcFieldValue>> {
            match $self.get_field(&WarcFieldName::$target) {
                None => None,
                Some(value) => {
                    Some(
                        match value {
                            WarcFieldValue::$value_ident(inner) => {
                                Ok(inner)
                            }
                            other => {
                                Err(other)
                            }
                        }
                    )
                }
            }
        }
    };

    (required@$target: ident with $name: ident($self: ident) -> $value_ident: ident as $typ: ty) => {

        pub fn $name<'a>(&'a $self) -> Result<&'a $typ, RequiredFieldError<'a>> {
            match $self.get_field(&WarcFieldName::$target) {
                None => Err(RequiredFieldError::NotFound(WarcFieldName::$target)),
                Some(value) => {
                    match value {
                        WarcFieldValue::$value_ident(inner) => {
                            Ok(inner)
                        }
                        other => {
                            Err(RequiredFieldError::WrongType(WarcFieldName::$target, other))
                        }
                    }
                }
            }
        }
    };
}

macro_rules! create_setter_and_getter {
    ($target: ident with $name: ident($self: ident, $var_name: ident: $typ: ty) -> $value_typ: ident; $name_get: ident@$opt_or_req: tt) => {
        create_setter!($target with $name($self, $var_name: $typ) -> $value_typ);
        create_getter!($opt_or_req@$target with $name_get($self) -> $value_typ as $typ);
    };
    (general@$target: ident with $name: ident($self: ident); $name_get: ident@$opt_or_req: tt) => {
        create_setter!(general@$target with $name($self));
        create_getter!($opt_or_req@$target with $name_get($self) -> General as GeneralFieldValue);
    };

    (general@$target: ident with $name: ident($self: ident) and fn check($value: ident) $check_block: block; $name_get: ident@$opt_or_req: tt) => {
        create_setter!(general@$target with $name($self) and fn check($value) $check_block);
        create_getter!($opt_or_req@$target with $name_get($self) -> General as GeneralFieldValue);
    };
    (uri@$target: ident with $name: ident($self: ident); $name_get: ident@$opt_or_req: tt) => {
        create_setter!(uri@$target with $name($self));
        create_getter!($opt_or_req@$target with $name_get($self) -> UriLike as UriLikeFieldValue);
    };

    ($target: ident with $name: ident($self: ident, $var_name: ident: $typ: ty) -> $value_typ: ident; @$opt_or_req: tt) => {
        create_setter!($target with $name($self, $var_name: $typ) -> $value_typ);
        paste!(
            create_getter!($opt_or_req@$target with [<get_ $name>]($self) -> $value_typ as $typ);
        );
    };
    (general@$target: ident with $name: ident($self: ident); @$opt_or_req: tt) => {
        create_setter!(general@$target with $name($self));
        paste!(
            create_getter!($opt_or_req@$target with [<get_ $name>]($self) -> General as GeneralFieldValue);
        );
    };

    (general@$target: ident with $name: ident($self: ident) and fn check($value: ident) $check_block: block; @$opt_or_req: tt) => {
        create_setter!(general@$target with $name($self) and fn check($value) $check_block);
        paste!(
            create_getter!($opt_or_req@$target with [<get_ $name>]($self) -> General as GeneralFieldValue);
        );
    };
    (uri@$target: ident with $name: ident($self: ident); @$opt_or_req: tt) => {
        create_setter!(uri@$target with $name($self));
        paste!(
            create_getter!($opt_or_req@$target with [<get_ $name>]($self) -> UriLike as UriLikeFieldValue);
        );

    };
}

impl WarcHeader {
    pub fn new() -> Self {
        Self {
            version: None,
            warc_headers: HashMap::default(),
        }
    }

    pub fn with_version(version: String) -> Self {
        Self {
            version: Some(version),
            warc_headers: HashMap::default(),
        }
    }

    // A WARC-Record-ID is an identifier assigned to the current record that is globally unique for
    // its period of intended use. No identifier scheme is mandated by this specification,
    // but each WARC-Record-ID shall be a legal URI and clearly indicate a documented and
    // registered scheme to which it conforms (e.g. via a URI scheme prefix such as “http:” or “urn:”).
    // Care should be taken to ensure that this value is written with no internal white space.
    create_setter_and_getter!(uri@WarcRecordId with warc_record_id(self); @required);
    create_setter_and_getter!(uri@ConcurrentTo with concurrent_to(self); @optional);
    create_setter_and_getter!(uri@RefersTo with refers_to(self); @optional);
    create_setter_and_getter!(uri@RefersToTargetUri with refers_to_target(self); @optional);
    create_setter_and_getter!(uri@TargetURI with target_uri(self); @optional);
    create_setter_and_getter!(uri@WarcInfoID with info_id(self); @optional);
    create_setter_and_getter!(uri@Profile with profile(self); @optional);
    create_setter_and_getter!(uri@SegmentOriginID with segment_origin_id(self); @optional);

    #[cfg(feature = "atra-fieldnames")]
    create_setter_and_getter!(Base64Encoded with atra_is_base64(self, is_base64: bool) -> Bool; @optional);

    create_setter_and_getter!(WarcType with warc_type(self, record_type: WarcRecordType) -> WarcRecordType; @required);

    #[cfg(feature = "atra-fieldnames")]
    create_setter_and_getter!(ContentEncoding with atra_content_encoding(self, encoding: &'static Encoding) -> Encoding; @optional);

    create_setter_and_getter!(Date with date(self, date: time::OffsetDateTime) -> Date; @required);
    create_setter_and_getter!(RefersToDate with referes_to_date(self, date: time::OffsetDateTime) -> Date; @optional);

    // Number of octets in the block
    create_setter_and_getter!(ContentLength with content_length(self, content_length: u64) -> Number; @required);

    #[cfg(feature = "atra-fieldnames")]
    create_setter_and_getter!(HeaderLength with header_length(self, header_length: u64) -> Number; @optional);
    create_setter_and_getter!(SegmentNumber with segment_number(self, segment_number: u64) -> Number; @optional);
    // Sum of all octets in all segments
    create_setter_and_getter!(SegmentTotalLength with segment_total_length(self, total_length: u64) -> Number; @optional);

    create_setter_and_getter!(ContentType with content_type(self, content_type: MediaType) -> ContentType; @optional);
    create_setter_and_getter!(IdentifiedPayloadType with indentified_payload_type(self, content_type: MediaType) -> ContentType; @optional);

    create_setter_and_getter!(Truncated with truncated_reason(self, reason: TruncatedReason) -> TruncatedReason; @optional);

    create_setter_and_getter!(IPAddress with ip_address(self, ip: IpAddr) -> IPAddress; @optional);

    // The WARC-Block-Digest is an optional parameter indicating the algorithm name and calculated value of a digest applied to the full block of the record.
    //
    // WARC-Block-Digest = "WARC-Block-Digest" ":" labelled-digest
    // labelled-digest   = algorithm ":" digest-value
    // algorithm         = token
    // digest-value      = token
    create_setter_and_getter!(general@BlockDigest with block_digest(self) and fn check(value) {
        if value.contains(b":") {
            Ok(value)
        } else {
            Err(WarcHeaderValueError::DigestIsMissingAlgorithm)
        }
    }; @optional);

    create_setter_and_getter!(general@PayloadDigest with payload_digest(self) and fn check(value) {
        if value.contains(b":") {
            Ok(value)
        } else {
            Err(WarcHeaderValueError::DigestIsMissingAlgorithm)
        }
    }; @optional);

    create_setter_and_getter!(general@Filename with file_name(self); @optional);

    #[cfg(feature = "atra-fieldnames")]
    create_setter_and_getter!(general@ExternalBinFile with external_bin_file(self); @optional);

    /// Unsafe setter, allows to basically set everything with every value
    pub unsafe fn unchecked_field(
        &mut self,
        key: WarcFieldName,
        value: WarcFieldValue,
    ) -> Option<WarcFieldValue> {
        self.warc_headers.insert(key, value)
    }

    /// Returns the value to the field if any
    pub fn get_field<Q: ?Sized>(&self, k: &Q) -> Option<&WarcFieldValue>
    where
        WarcFieldName: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.warc_headers.get(k)
    }

    /// Returns Err if this is not valid
    pub fn is_valid(&self) -> Result<(), Vec<WarcFieldName>> {
        let data = [
            WarcFieldName::WarcRecordId,
            WarcFieldName::ContentLength,
            WarcFieldName::Date,
            WarcFieldName::WarcType,
        ]
        .into_iter()
        .filter(|it| !self.warc_headers.contains_key(it))
        .collect_vec();

        if data.is_empty() {
            Ok(())
        } else {
            Err(data)
        }
    }

    /// Writes the warc header.
    /// Checks the validity of the header.
    /// If [append_tailing_newline] is not set, the '\r\n' has to be set manually.
    /// Returns the number of bytes written
    pub fn write_to(
        &self,
        out: &mut impl Write,
        append_tailing_newline: bool,
    ) -> Result<usize, WarcHeaderWriteError> {
        if let Err(missing) = self.is_valid() {
            return Err(WarcHeaderWriteError::MandatoryValuesAreMissing(missing));
        }
        Ok(unsafe { self.write_to_unchecked(out, append_tailing_newline)? })
    }

    /// Writes the warc header to [out] without checking the validity of the header.
    /// If [append_tailing_newline] is not set, the '\r\n' has to be set manually.
    /// Returns the number of bytes written
    pub unsafe fn write_to_unchecked(
        &self,
        out: &mut impl Write,
        append_tailing_newline: bool,
    ) -> Result<usize, WarcHeaderWriteError> {
        let mut written = 0usize;

        written += if let Some(ref v) = self.version {
            out.write(v.as_bytes())?
        } else {
            out.write(WARC_VERSION)?
        };
        written += out.write(b"\r\n")?;

        for (k, v) in self.warc_headers.iter() {
            written += out.write(k.as_ref().as_bytes())?;
            written += out.write(b":")?;
            written += v.write_to(out)?;
            written += out.write(b"\r\n")?;
        }
        if append_tailing_newline {
            written += out.write(b"\r\n")?;
        }
        Ok(written)
    }
}

impl Display for WarcHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = Vec::new();
        // Never fails
        unsafe { self.write_to_unchecked(&mut s, false).unwrap() };
        let s = unsafe { String::from_utf8_unchecked(s) };
        f.write_str(&s)
    }
}
