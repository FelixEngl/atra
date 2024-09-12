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

/// Used http-serde = "2.0" for base
pub mod header_map {
    use reqwest::header::{GetAll, HeaderName, InvalidHeaderValue};
    use reqwest::header::{HeaderMap, HeaderValue};
    use serde::{Deserialize};
    use serde::de::{Deserializer, Error, MapAccess, Unexpected, Visitor};
    use serde::{Serialize, Serializer};
    use std::borrow::Cow;
    use std::fmt;
    use smallvec::SmallVec;

    pub struct ToSeq<'a>(pub GetAll<'a, HeaderValue>);

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(untagged)]
    enum SecureHeaderValue<'a> {
        String(Cow<'a, str>),
        Opaque { opaque: Cow<'a, [u8]> }
    }

    #[derive(Deserialize, Serialize)]
    #[serde(untagged)]
    enum OneOrMore<'a> {
        One(SecureHeaderValue<'a>),
        Many(SmallVec<[SecureHeaderValue<'a>; 8]>),
    }

    impl SecureHeaderValue<'_> {
        fn as_header_value(&self) -> Result<HeaderValue, InvalidHeaderValue> {
            match self {
                SecureHeaderValue::String(value) => {
                    HeaderValue::from_str(&value)
                }
                SecureHeaderValue::Opaque{ opaque } => {
                    HeaderValue::from_bytes(&opaque)
                }
            }
        }
    }

    impl<'a> From<&'a HeaderValue> for SecureHeaderValue<'a> {
        fn from(value: &'a HeaderValue) -> Self {
            if let Ok(s) = value.to_str() {
                Self::String(Cow::Borrowed(s))
            } else {
                Self::Opaque {
                    opaque: Cow::Borrowed(value.as_bytes())
                }
            }
        }
    }

    impl<'a> Serialize for ToSeq<'a> {
        fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            if ser.is_human_readable() {
                let data =
                    self
                        .0
                        .iter()
                        .map(|value| SecureHeaderValue::from(value))
                        .collect::<SmallVec<[SecureHeaderValue; 8]>>();
                if ser.is_human_readable() && data.len() == 1 {
                    return data[0].serialize(ser)
                }
                data.serialize(ser)
            } else {
                self.0
                    .iter()
                    .map(|value| value.as_bytes()).collect::<SmallVec<[&[u8]; 8]>>()
                    .serialize(ser)
            }

        }
    }

    /// Implementation detail. Use derive annotations instead.
    pub fn serialize<S: Serializer>(headers: &HeaderMap, ser: S) -> Result<S::Ok, S::Error> {
        ser.collect_map(
            headers
                .keys()
                .map(|k| (k.as_str(), ToSeq(headers.get_all(k)))),
        )
    }



    pub struct HeaderMapVisitor {
        pub is_human_readable: bool,
    }

    impl<'de> Visitor<'de> for HeaderMapVisitor {
        type Value = HeaderMap;

        // Format a message stating what data this Visitor expects to receive.
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("lots of things can go wrong with HeaderMap")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
        {
            let mut map = HeaderMap::with_capacity(access.size_hint().unwrap_or(0));

            if !self.is_human_readable {
                while let Some((key, data)) = access.next_entry::<Cow<str>, SmallVec<[Cow<[u8]>; 8]>>()? {
                    let key = HeaderName::from_bytes(key.as_bytes())
                        .map_err(|_| Error::invalid_value(Unexpected::Str(&key), &self))?;
                    for val in data {
                        let val = HeaderValue::from_bytes(&val).map_err(
                            |_| Error::invalid_value(Unexpected::Bytes(&val), &self)
                        )?;
                        map.append(&key, val);
                    }
                }
            } else {
                while let Some((key, val)) = access.next_entry::<Cow<str>, OneOrMore>()? {
                    let key = HeaderName::from_bytes(key.as_bytes())
                        .map_err(|_| Error::invalid_value(Unexpected::Str(&key), &self))?;
                    match val {
                        OneOrMore::One(val) => {
                            let val = val
                                .as_header_value()
                                .map_err(|_err|
                                    match val {
                                        SecureHeaderValue::String(value) => {
                                            Error::invalid_value(Unexpected::Str(&value), &self)
                                        }
                                        SecureHeaderValue::Opaque { opaque } => {
                                            Error::invalid_value(Unexpected::Bytes(&opaque), &self)
                                        }
                                    }
                                )?;

                            map.insert(key, val);
                        }
                        OneOrMore::Many(arr) => {
                            for val in arr {
                                let val = val
                                    .as_header_value()
                                    .map_err(|_err|
                                        match val {
                                            SecureHeaderValue::String(value) => {
                                                Error::invalid_value(Unexpected::Str(&value), &self)
                                            }
                                            SecureHeaderValue::Opaque { opaque } => {
                                                Error::invalid_value(Unexpected::Bytes(&opaque), &self)
                                            }
                                        }
                                    )?;
                                map.append(&key, val);
                            }
                        }
                    };
                }
            }
            Ok(map)
        }
    }

    /// Implementation detail.
    pub fn deserialize<'de, D>(de: D) -> Result<HeaderMap, D::Error>
        where
            D: Deserializer<'de>,
    {
        let is_human_readable = de.is_human_readable();
        de.deserialize_map(HeaderMapVisitor { is_human_readable })
    }
}

pub mod optional_header_map {
    use std::fmt::Formatter;
    use reqwest::header::HeaderMap;
    use serde::{Deserializer, Serialize, Serializer};
    use serde::de::{Error, Visitor};
    use crate::toolkit::header_map_extensions::header_map;

    #[derive(Serialize)]
    #[repr(transparent)]
    #[serde(transparent)]
    struct SomeHeaderMap<'a>(
        #[serde(with = "header_map")]
        &'a HeaderMap
    );

    pub fn serialize<S: Serializer>(headers: &Option<HeaderMap>, ser: S) -> Result<S::Ok, S::Error> {
        if let Some(headers) = headers {
            ser.serialize_some(&SomeHeaderMap(headers))
        } else {
            ser.serialize_none()
        }
    }

    struct OptionalHeaderMapVisitor;

    impl<'de> Visitor<'de> for OptionalHeaderMapVisitor {
        type Value = Option<HeaderMap>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("lots of things can go wrong with Optional<HeaderMap>")
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
            Ok(Some(header_map::deserialize(deserializer)?))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> where E: Error {
            return Ok(None)
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<HeaderMap>, D::Error>
        where
            D: Deserializer<'de>,
    {
        de.deserialize_option(OptionalHeaderMapVisitor)
    }
}
