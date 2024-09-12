// Copyright 2024 Felix Engl
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

#[macro_export]
macro_rules! next_from_seq {
    ($self: ident, $seq: ident, $len: expr) => {
        match $seq.next_element()? {
            Some(value) => value,
            None => return Err(Error::invalid_length($len, &$self)),
        }
    };
}

#[macro_export]
macro_rules! next_key_from_map {
    ($self: ident, $map: ident, $len: expr, $exp: expr) => {
        match $map.next_key::<&str>()? {
            Some(value) => {
                if !$exp.contains(&value) {
                    return Err(Error::unknown_field(value, $exp));
                } else {
                    value
                }
            }
            None => return Err(Error::invalid_length($len, &$self)),
        }
    };
}

/// For `http::StatusCode`
///
/// `#[serde(with = "http_serde::status_code")]`
pub mod status_code {
    use reqwest::StatusCode;
    use serde::de;
    use serde::de::{Unexpected, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    /// Implementation detail. Use derive annotations instead.
    pub fn serialize<S: Serializer>(status: &StatusCode, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_u16(status.as_u16())
    }

    struct StatusVisitor;

    impl<'de> Visitor<'de> for StatusVisitor {
        type Value = StatusCode;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "valid status code")
        }

        fn visit_i32<E: de::Error>(self, val: i32) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_i16<E: de::Error>(self, val: i16) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_u8<E: de::Error>(self, val: u8) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_u32<E: de::Error>(self, val: u32) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_i64<E: de::Error>(self, val: i64) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_u64<E: de::Error>(self, val: u64) -> Result<Self::Value, E> {
            self.visit_u16(val as u16)
        }

        fn visit_u16<E: de::Error>(self, val: u16) -> Result<Self::Value, E> {
            StatusCode::from_u16(val)
                .map_err(|_| de::Error::invalid_value(Unexpected::Unsigned(val.into()), &self))
        }
    }

    /// Implementation detail.
    pub fn deserialize<'de, D>(de: D) -> Result<StatusCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        de.deserialize_u16(StatusVisitor)
    }
}
