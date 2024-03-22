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

use std::cmp::{Ordering};
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use case_insensitive_string::CaseInsensitiveString;
use itertools::{EitherOrBoth, Itertools, Position};
use reqwest::IntoUrl;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeStruct, SerializeTupleStruct};
use url::Url;
use crate::core::depth::DepthDescriptor;
use crate::{next_from_seq, next_key_from_map};


/// Represents an url with knowledge about its depth and raw representation.
/// The equals and hash methods only consider the [parsed_url].
/// The order is determined by [depth] and the equality of the [parsed_url].
#[derive(Debug, Eq, Clone)]
pub struct UrlWithDepth {
    /// Describes the depth of the url.
    pub depth: DepthDescriptor,
    /// The parsed url, may add / at the end
    pub url: Url,
}

impl UrlWithDepth {

    /// Creates an url with depth from
    pub fn from_seed<U: IntoUrl>(url: U) -> Result<Self, url::ParseError> {
        Self::new(DepthDescriptor::ZERO, url)
    }

    /// Parses the url and associates it to a [depth]
    pub fn new<U: IntoUrl>(depth: DepthDescriptor, url: U) -> Result<Self, url::ParseError> {
        let url = url.as_str().trim();
        let mut parsed_url = Url::parse(url)?;
        parsed_url.set_fragment(None);
        Ok(
            Self {
                url: parsed_url,
                depth
            }
        )
    }

    /// Returns the scheme of the underlying url
    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    fn new_with_base(base: &UrlWithDepth, url: Url) -> Result<Self, url::ParseError> {
        let mut depth = base.depth;
        if let Some(host) = url.host_str() {
            if let Some(base_host) = base.url.host_str() {
                if host.eq_ignore_ascii_case(base_host) {
                    depth.depth_on_website += 1;
                } else {
                    depth.depth_on_website = 0;
                    depth.distance_to_seed += 1;
                }
            } else {
                depth.depth_on_website = 0;
                depth.distance_to_seed += 1;
            }
        } else {
            depth.depth_on_website += 1;
        }
        depth.total_distance_to_seed += 1;


        Ok(
            Self {
                depth,
                url,
            }
        )
    }

    /// Creates a new url with [base] as base for [raw_url] if needed.
    pub fn with_base<U: IntoUrl>(base: &UrlWithDepth, raw_url: U) -> Result<Self, url::ParseError> {
        let raw_url = raw_url.as_str().trim();
        let mut parsed_url = Url::options().base_url(Some(&base.url)).parse(raw_url)?;
        parsed_url.set_fragment(None);
        Self::new_with_base(base, parsed_url)
    }

    /// Creates a new url but behaves like if found a base url
    pub fn new_like_with_base<U: IntoUrl>(base: &UrlWithDepth, raw_url: U) -> Result<Self, url::ParseError> {
        let raw_url = raw_url.as_str().trim();
        let mut parsed_url = Url::parse(raw_url)?;
        parsed_url.set_fragment(None);
        Self::new_with_base(base, parsed_url)
    }


    /// Returns the string representation of the `parsed_url`
    pub fn as_str(&self) -> &str {
        return self.url.as_str()
    }

    /// Checks
    pub fn is_exactly_same_as(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || (
            self.depth == other.depth
                && self.url == other.url
        )
    }

    /// Compares if the urls are the same
    pub fn is_same_url_as<U: IntoUrl>(&self, other: U) -> bool {
        other.into_url().map(|url| self.url == url).unwrap_or(false)
    }

    /// Checks if the domain of the url fits the provided [domain].
    /// Returns None if the parsed url does not provide a domain.
    pub fn domain_is_equal_to(&self, other_domain: &CaseInsensitiveString) -> Option<bool>{
        Some(self.url.domain()?.eq_ignore_ascii_case(other_domain.as_ref()))
    }

    /// Extracts the domain of the `parsed_url`.
    pub fn domain(&self) -> Option<CaseInsensitiveString> {
        Some(CaseInsensitiveString::new(self.url.domain()?))
    }

    /// Returns the name of the domain without any suffix.
    /// Cleanup depends on the public suffix list .
    pub fn domain_name(&self) -> Option<CaseInsensitiveString> {
        Some(
            CaseInsensitiveString::new(
                psl::domain(
                    self.url
                        .host_str()?
                        .as_bytes()
                )?.as_bytes()
            )
        )
    }

    /// Returns a url without path, query and fragment.
    pub fn clean_url(&self) -> Url {
        let mut target = self.url.clone();
        target.set_path("");
        target.set_query(None);
        target.set_fragment(None);
        target
    }

    /// Returns true if the base of this url is [base].
    pub fn has_as_base(&self, base: &UrlWithDepth) -> bool {
        self.has_url_as_base(&base.url)
    }

    /// Returns true if the base of this url is [base].
    pub fn has_url_as_base(&self, base: &Url) -> bool {
        if base.cannot_be_a_base() {
            return false
        }
        base.make_relative(&self.url).is_some()
    }
}

impl FromStr for UrlWithDepth {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_seed(s)
    }
}

impl Display for UrlWithDepth {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, \"{}\")", self.depth, self.url.as_str())
    }
}

impl Hash for UrlWithDepth {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state)
    }
}

impl PartialEq for UrlWithDepth {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Ord for UrlWithDepth {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.url == other.url {
            match self.depth.distance_to_seed.cmp(&other.depth.distance_to_seed) {
                Ordering::Equal => self.depth.depth_on_website.cmp(&other.depth.depth_on_website),
                other => other
            }
        } else {
            for (position, value) in self.url.as_str().as_bytes().iter().zip_longest(other.url.as_str().as_bytes().iter()).with_position() {
                match position {
                    Position::First | Position::Middle => {
                        match value {
                            EitherOrBoth::Both(a, b) => {
                                match a.cmp(b) {
                                    Ordering::Less => { return Ordering::Less }
                                    Ordering::Greater => { return Ordering::Greater }
                                    _ => {}
                                }
                            }
                            EitherOrBoth::Left(_) => {
                                return Ordering::Less
                            }
                            EitherOrBoth::Right(_) => {
                                return Ordering::Greater
                            }
                        }
                    }
                    Position::Last | Position::Only => {
                        return match value {
                            EitherOrBoth::Both(a, b) => {
                                match a.cmp(b) {
                                    Ordering::Less => Ordering::Less,
                                    Ordering::Greater => Ordering::Greater,
                                    _ => {
                                        match self.depth.distance_to_seed.cmp(&other.depth.distance_to_seed) {
                                            Ordering::Equal => self.depth.depth_on_website.cmp(&other.depth.depth_on_website),
                                            other => other
                                        }
                                    }
                                }
                            }
                            EitherOrBoth::Left(_) => {
                                Ordering::Less
                            }
                            EitherOrBoth::Right(_) => {
                                Ordering::Greater
                            }
                        }
                    }
                }
            }
            match self.depth.distance_to_seed.cmp(&other.depth.distance_to_seed) {
                Ordering::Equal => self.depth.depth_on_website.cmp(&other.depth.depth_on_website),
                other => other
            }
        }
    }
}

impl PartialOrd for UrlWithDepth {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for UrlWithDepth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        if serializer.is_human_readable() {
            let mut seq = serializer.serialize_struct("UrlWithDepth", 2)?;
            seq.serialize_field("depth", &self.depth)?;
            seq.serialize_field("url", &self.url)?;
            seq.end()
        } else {
            let mut seq = serializer.serialize_tuple_struct("UrlWithDepth", 2)?;
            seq.serialize_field(&self.depth)?;
            seq.serialize_field(&self.url)?;
            seq.end()
        }

    }
}

impl Deref for UrlWithDepth {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.url
    }
}


impl AsRef<Url> for UrlWithDepth {
    fn as_ref(&self) -> &Url {
        &self.url
    }
}

impl AsRef<[u8]> for UrlWithDepth {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_ref()
    }
}


struct UrlWithDepthVisitor;

const FIELDS: [&'static str; 2] = ["depth", "url"];

impl<'de> Visitor<'de> for UrlWithDepthVisitor {
    type Value = UrlWithDepth;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter,
               "An url with depth is stored as a (DepthDescriptor, CompactString, Url) tuple. \
               The CIString hast to be parseable to a Url.")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let depth: DepthDescriptor = next_from_seq!(self, seq, 2);
        let url: Url = next_from_seq!(self, seq, 2);
        Ok(
            UrlWithDepth {
                depth,
                url
            }
        )
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error> where A: MapAccess<'de> {

        let mut depth: Option<DepthDescriptor> = None;
        let mut url: Option<Url> = None;

        for _ in 0..2 {
            let key = next_key_from_map!(self, map, 2, &FIELDS);
            match key {
                "depth" => depth = Some(map.next_value()?),
                "url" => url = Some(map.next_value()?),
                illegal => return Err(Error::unknown_field(illegal, &FIELDS))
            }
        }

        let depth = depth.ok_or(Error::missing_field("depth"))?;
        let url = url.ok_or(Error::missing_field("url"))?;
        Ok(
            UrlWithDepth {
                depth,
                url
            }
        )
    }
}

impl<'de> Deserialize<'de> for UrlWithDepth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_struct(
                "UrlWithDepth",
                &FIELDS,
                UrlWithDepthVisitor
            )
        } else {
            deserializer.deserialize_tuple(3, UrlWithDepthVisitor)
        }
    }
}


#[cfg(test)]
mod test {
    use crate::core::depth::DepthDescriptor;
    use crate::core::UrlWithDepth;

    #[test]
    fn base_only_changes_if_not_given(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.siemens.com"), created.url.host_str());
        let created = UrlWithDepth::with_base(&base, "lookup?v=20").unwrap();
        assert_eq!(Some("www.example.com"), created.url.host_str());
        assert_eq!("/lookup", created.url.path());
    }

    #[test]
    fn depth_on_website_goes_up_if_on_same_domain(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.example.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.example.com"), created1.url.host_str());
        assert_eq!(base.depth + (1, 0, 1), created1.depth);
        let created2 = UrlWithDepth::with_base(&created1, "https://www.example.com/test?v=20").unwrap();
        assert_eq!(created1.depth + (1, 0, 1), created2.depth);
    }

    #[test]
    fn distance_to_seed_goes_up_if_not_same_domain(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.siemens.com"), created1.url.host_str());
        assert_eq!(DepthDescriptor::ZERO + (0,1,1), created1.depth);
        let created2 = UrlWithDepth::with_base(&created1, "https://www.siemens.com/test?v=20").unwrap();
        assert_eq!(created1.depth + (1, 0, 1), created2.depth);
        let created3 = UrlWithDepth::with_base(&created2, "https://www.google.com/test?v=20").unwrap();
        assert_eq!(DepthDescriptor::ZERO + (0, created2.depth.distance_to_seed + 1, created2.depth.total_distance_to_seed + 1), created3.depth);
    }

    #[test]
    fn can_serialize_and_deserialize_nonhuman() {
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        let serialized = bincode::serialize(&created1).unwrap();
        let deserialized1: UrlWithDepth = bincode::deserialize(&serialized).unwrap();
        assert!(created1.is_exactly_same_as(&deserialized1), "Failed: \n  {:?}\n  !=\n  {:?}", created1, deserialized1)
    }

    #[test]
    fn can_serialize_and_deserialize_human() {
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        let serialized = serde_json::to_string(&created1).unwrap();
        let deserialized1: UrlWithDepth = serde_json::from_str(&serialized).unwrap();
        assert!(created1.is_exactly_same_as(&deserialized1), "Failed: \n  {:?}\n  !=\n  {:?}", created1, deserialized1)
    }
}