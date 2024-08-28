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

use std::borrow::Cow;
use std::cmp::{Ordering};
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use case_insensitive_string::CaseInsensitiveString;
use itertools::{EitherOrBoth, Itertools, Position};
use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};
use crate::core::depth::DepthDescriptor;
use crate::core::origin::{AtraUrlOrigin, AtraOriginProvider};
use crate::core::url::cleaner::SingleUrlCleaner;
use crate::core::url::atra_uri::{AtraUri, ParseError, HostComparisonError};

/// Represents an url with knowledge about its depth and raw representation.
/// The equals and hash methods only consider the [parsed_url].
/// The order is determined by [depth] and the equality of the [parsed_url].
#[derive(Debug, Eq, Clone, Serialize, Deserialize)]
pub struct UrlWithDepth {
    /// Describes the depth of the url.
    pub depth: DepthDescriptor,
    /// The parsed url, may add / at the end
    pub url: AtraUri,
}

impl UrlWithDepth {
    /// Creates a new [UrlWithDepth]
    pub fn new(depth: DepthDescriptor, mut url: AtraUri) -> Self {
        url.clean(SingleUrlCleaner::Fragment);
        Self {
            url,
            depth
        }
    }

    /// Creates an url with depth from
    pub fn from_seed<U: IntoUrl>(url: U) -> Result<Self, ParseError> {
        Ok(Self::new(DepthDescriptor::ZERO, url.as_str().parse::<AtraUri>()?))
    }

    #[inline(always)]
    pub fn url(&self) -> &AtraUri {
        &self.url
    }

    #[inline(always)]
    pub fn depth(&self) -> &DepthDescriptor {
        &self.depth
    }


    /// Returns the scheme of the underlying url
    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    fn create_new_calculate_depth_with_base(base: &UrlWithDepth, url: AtraUri) -> Result<Self, ParseError> {
        let mut depth = base.depth;
        
        match url.compare_hosts(&base.url) {
            Ok(true) => {
                depth.depth_on_website += 1;
            }
            Ok(false) | Err(HostComparisonError::NoHost { left_has_host: true, right_has_host: false }) => {
                depth.depth_on_website = 0;
                depth.distance_to_seed += 1;
            }
            Err(_) => {
                depth.depth_on_website += 1;
            }
        }

        // if let Some(host) = url.host_str() {
        //     if let Some(base_host) = base.url.host_str() {
        //         if host.eq_ignore_ascii_case(base_host) {
        //             depth.depth_on_website += 1;
        //         } else {
        //             depth.depth_on_website = 0;
        //             depth.distance_to_seed += 1;
        //         }
        //     } else {
        //         depth.depth_on_website = 0;
        //         depth.distance_to_seed += 1;
        //     }
        // } else {
        //     depth.depth_on_website += 1;
        // }
        depth.total_distance_to_seed += 1;

        Ok(
            Self {
                depth,
                url,
            }
        )
    }

    /// Creates a new url with [base] as base for [raw_url] if needed.
    pub fn with_base<U: IntoUrl>(base: &UrlWithDepth, url: U) -> Result<Self, ParseError> {
        let mut url = AtraUri::with_base(&base.url, url.as_str())?;
        url.clean(SingleUrlCleaner::Fragment);
        Self::create_new_calculate_depth_with_base(base, url)
    }

    /// Creates a new url but behaves like if found a base url
    pub fn new_like_with_base<U: IntoUrl>(base: &UrlWithDepth, url: U) -> Result<Self, ParseError> {
        let mut url: AtraUri = url.as_str().parse()?;
        url.clean(SingleUrlCleaner::Fragment);
        Self::create_new_calculate_depth_with_base(base, url)
    }

    /// Checks
    pub fn is_exactly_same_as(&self, other: &Self) -> bool {
        std::ptr::eq(self, other) || (
            self.depth == other.depth
                && self.url == other.url
        )
    }

    pub fn as_str(&self) -> Cow<str> {
        if let Some(s) = self.url.try_as_str() {
            Cow::Borrowed(s)
        } else {
            Cow::Owned(self.url.to_string())
        }
    }

    /// Extracts the domain of the `parsed_url`.
    pub fn domain(&self) -> Option<CaseInsensitiveString> {
        self.url.as_url()?.domain().map(CaseInsensitiveString::from)
    }

    /// Returns the name of the domain without any suffix.
    /// Cleanup depends on the public suffix list .
    pub fn domain_name(&self) -> Option<CaseInsensitiveString> {
        self.url.domain_name()
    }

    /// Returns a url without path, query and fragment.
    pub fn clean_url(&self) -> AtraUri {
        let mut target = self.url.clone();
        target.clean([SingleUrlCleaner::Fragment, SingleUrlCleaner::Query, SingleUrlCleaner::Password]);
        target
    }
}

impl AtraOriginProvider for UrlWithDepth {
    fn atra_origin(&self) -> Option<AtraUrlOrigin> {
        self.url.atra_origin()
    }
}

impl FromStr for UrlWithDepth {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_seed(s)
    }
}

impl Display for UrlWithDepth {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, \"{}\")", self.depth, self.url)
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
            for (position, value) in self.url.as_bytes().iter().zip_longest(other.url.as_bytes().iter()).with_position() {
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


impl Deref for UrlWithDepth {
    type Target = AtraUri;

    fn deref(&self) -> &Self::Target {
        &self.url
    }
}


impl AsRef<AtraUri> for UrlWithDepth {
    fn as_ref(&self) -> &AtraUri {
        &self.url
    }
}

impl AsRef<[u8]> for UrlWithDepth {
    fn as_ref(&self) -> &[u8] {
        self.url.as_bytes()
    }
}


#[cfg(test)]
mod test {
    use crate::core::depth::DepthDescriptor;
    use crate::core::origin::AtraOriginProvider;
    use crate::core::UrlWithDepth;

    #[test]
    fn base_only_changes_if_not_given(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.siemens.com".into()), created.url.atra_origin());
        let created = UrlWithDepth::with_base(&base, "lookup?v=20").unwrap();
        assert_eq!(Some("www.example.com".into()), created.url.atra_origin());
        assert_eq!(Some("/lookup"), created.url.path());
    }

    #[test]
    fn depth_on_website_goes_up_if_on_same_domain(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.example.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.example.com".into()), created1.url.atra_origin());
        assert_eq!(base.depth + (1, 0, 1), created1.depth);
        let created2 = UrlWithDepth::with_base(&created1, "https://www.example.com/test?v=20").unwrap();
        assert_eq!(created1.depth + (1, 0, 1), created2.depth);
    }

    #[test]
    fn distance_to_seed_goes_up_if_not_same_domain(){
        let base = UrlWithDepth::from_seed("https://www.example.com/").unwrap();
        let created1 = UrlWithDepth::with_base(&base, "https://www.siemens.com/lookup?v=20").unwrap();
        assert_eq!(Some("www.siemens.com".into()), created1.url.atra_origin());
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