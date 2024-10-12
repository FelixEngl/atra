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

use super::origin::{AtraOriginProvider, AtraUrlOrigin};
use crate::toolkit::extension_extractor::extract_file_extensions_from_file_name;
use crate::toolkit::CaseInsensitiveString;
use crate::url::cleaner::AtraUrlCleaner;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use thiserror::Error;
use url::Url;
use warc::field::{ToUriLikeFieldValue, UriLikeFieldValue};

/// A separated type for URL to prepare for supporting different kind of URLs
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub enum AtraUri {
    Url(Url),
}

#[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
pub enum HostComparisonError {
    /// Only for [AtraUri::Url], returned when there is no host.
    #[error("The left has {} and right has {}!", if *(.left_has_host) {"host"} else {"no host"}, if *(.right_has_host) {"host"} else {"no host"})]
    NoHost {
        left_has_host: bool,
        right_has_host: bool,
    },
}

/// Errors when converting something to an [AtraUri]
#[derive(Debug, Clone, Error)]
pub enum ParseError {
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),
}

impl AtraUri {
    pub fn parse<T: AsRef<str>>(url: T) -> Result<Self, ParseError> {
        url.as_ref().parse()
    }

    pub fn with_base<U: AsRef<str>>(base: &Self, target: U) -> Result<Self, ParseError> {
        let target = target.as_ref();
        match base {
            AtraUri::Url(base) => Ok(AtraUri::Url(
                Url::options().base_url(Some(base)).parse(target)?,
            )),
        }
    }

    /// Returns the scheme of the underlying url
    pub fn scheme(&self) -> &str {
        match self {
            AtraUri::Url(value) => value.scheme(),
        }
    }

    #[inline(always)]
    pub fn clean<C: AtraUrlCleaner>(&mut self, cleaner: C) {
        cleaner.clean(self)
    }

    /// Returns the path
    pub fn path(&self) -> Option<&str> {
        match self {
            AtraUri::Url(value) => Some(value.path()),
        }
    }

    pub fn file_name(&self) -> Option<Cow<str>> {
        match self { AtraUri::Url(value) => {
            Some(Cow::Borrowed(value.path_segments()?.last()?))
        } }
    }

    /// Returns all file endings of the url
    pub fn get_file_endings(&self) -> Option<Vec<&str>> {
        match self {
            AtraUri::Url(value) => {
                let last = value.path_segments()?.last()?;
                extract_file_extensions_from_file_name(last)
            }
        }
    }

    /// Returns true if the hosts are similar
    pub fn same_host(&self, other: &Self) -> bool {
        match self {
            AtraUri::Url(a) => match other {
                AtraUri::Url(b) => a.host() == b.host(),
            },
        }
    }

    /// Returns true if the hosts are similar
    pub fn same_host_url(&self, other: &Url) -> bool {
        match self {
            AtraUri::Url(a) => a.host() == other.host(),
        }
    }

    /// If this URL has a host and it is a domain name (not an IP address), return it.
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::url::atra_uri::*;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = AtraUri::parse("https://127.0.0.1/")?;
    /// assert_eq!(url.domain(), None);
    ///
    /// let url = AtraUri::parse("mailto:rms@example.net")?;
    /// assert_eq!(url.domain(), None);
    ///
    /// let url = AtraUri::parse("https://example.com/")?;
    /// assert_eq!(url.domain(), Some("example.com".to_string()));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn domain(&self) -> Option<String> {
        match self {
            AtraUri::Url(value) => value.domain().map(|value| value.to_lowercase()),
        }
    }

    /// Return the string representation of the host (domain or IP address) for this URL, if any.
    ///
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    /// IPv6 addresses are given between `[` and `]` brackets.
    ///
    /// Cannot-be-a-base URLs (typical of `data:` and `mailto:`) and some `file:` URLs
    /// don’t have a host.
    ///
    /// See also the `host` method.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::url::atra_uri::*;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let url = AtraUri::parse("https://127.0.0.1/index.html")?;
    /// assert_eq!(url.host(), Some("127.0.0.1".to_string()));
    ///
    /// let url = AtraUri::parse("ftp://rms@example.com")?;
    /// assert_eq!(url.host(), Some("example.com".to_string()));
    ///
    /// let url = AtraUri::parse("unix:/run/foo.socket")?;
    /// assert_eq!(url.host(), None);
    ///
    /// let url = AtraUri::parse("data:text/plain,Stuff")?;
    /// assert_eq!(url.host(), None);
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn host(&self) -> Option<CaseInsensitiveString> {
        match self {
            AtraUri::Url(value) => value.host_str().map(|value| value.into()),
        }
    }

    /// Returns the name of the domain without any suffix.
    /// Cleanup depends on the public suffix list.
    pub fn domain_name(&self) -> Option<CaseInsensitiveString> {
        match self {
            AtraUri::Url(value) => Some(CaseInsensitiveString::from(
                psl::domain(value.host_str()?.as_bytes())?.as_bytes(),
            )),
        }
    }

    /// Returns it as string
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AtraUri::Url(value) => value.as_str().as_bytes(),
        }
    }

    /// Compares the host of two [AtraUri]s. Fails if they are somewhat not comparable.
    pub fn compare_hosts(&self, other: &Self) -> Result<bool, HostComparisonError> {
        #[inline(always)]
        fn compare_url(a: &Url, b: &Url) -> Result<bool, HostComparisonError> {
            if let Some(host_a) = a.host_str() {
                if let Some(host_b) = b.host_str() {
                    Ok(host_a.eq_ignore_ascii_case(host_b))
                } else {
                    Err(HostComparisonError::NoHost {
                        left_has_host: true,
                        right_has_host: false,
                    })
                }
            } else {
                Err(HostComparisonError::NoHost {
                    left_has_host: false,
                    right_has_host: b.has_host(),
                })
            }
        }

        match self {
            AtraUri::Url(a) => match other {
                AtraUri::Url(b) => compare_url(a, b),
            },
        }
    }

    /// Triesto return the underlying URL. Returns None if it fails.
    pub fn as_url(&self) -> Option<&Url> {
        match self {
            AtraUri::Url(value) => Some(value),
        }
    }

    pub fn as_mut_url(&mut self) -> Option<&mut Url> {
        match self {
            AtraUri::Url(value) => Some(value),
        }
    }

    /// Returns the str representation, iff the value can be represented as str
    pub fn try_as_str(&self) -> Option<&str> {
        match self {
            AtraUri::Url(value) => Some(value.as_str()),
        }
    }

    /// Returns either a str or the result of the to_string
    pub fn as_str(&self) -> Cow<str> {
        match self.try_as_str() {
            None => Cow::Owned(self.to_string()),
            Some(value) => Cow::Borrowed(value),
        }
    }

    /// Returns the file extension.
    pub fn file_extension(&self) -> Option<&str> {
        match self {
            AtraUri::Url(value) => {
                let path = value.path();
                let found = path.rfind('.')?;
                Some(&path[found..])
            }
        }
    }
}

impl AtraOriginProvider for AtraUri {
    fn atra_origin(&self) -> Option<AtraUrlOrigin> {
        match self {
            AtraUri::Url(value) => value.atra_origin(),
        }
    }
}

impl FromStr for AtraUri {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.parse::<Url>()?.into())
    }
}

impl AsRef<[u8]> for AtraUri {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<Url> for AtraUri {
    #[inline]
    fn from(value: Url) -> Self {
        Self::Url(value)
    }
}
impl<'a> TryFrom<&'a str> for AtraUri {
    type Error = ParseError;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Ok(AtraUri::from_str(value)?)
    }
}

impl TryFrom<String> for AtraUri {
    type Error = ParseError;
    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl<'a> TryFrom<&'a String> for AtraUri {
    type Error = ParseError;
    #[inline]
    fn try_from(value: &'a String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl Display for AtraUri {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AtraUri::Url(value) => Display::fmt(value, f),
        }
    }
}

impl ToUriLikeFieldValue for AtraUri {
    fn to_uri_like_field_value(self) -> UriLikeFieldValue {
        self.to_string().parse().expect("This does never fail!")
    }
}

#[cfg(test)]
mod test {
    use crate::url::atra_uri::{AtraUri, HostComparisonError};
    use url::Url;

    #[test]
    fn spaces_do_not_matter() {
        let a: AtraUri = "https://www.example.com/"
            .parse()
            .expect("Expected a sucessfull parse!");
        let b: AtraUri = "  https://www.example.com/  "
            .parse()
            .expect("Expected a sucessfull parse!");
        let c: AtraUri = "  https://www.konto.example.com/  "
            .parse()
            .expect("Expected a sucessfull parse!");
        println!("{:?}", a.domain_name());
        println!("{:?}", c.domain_name());
        assert_eq!(a, b);
        assert_eq!(a.domain_name(), b.domain_name());
        assert_eq!(a.to_string(), b.to_string());
    }

    fn old_impl(base: &Url, url: &Url) -> Result<bool, HostComparisonError> {
        if let Some(host) = url.host_str() {
            if let Some(base_host) = base.host_str() {
                Ok(host.eq_ignore_ascii_case(base_host))
            } else {
                Err(HostComparisonError::NoHost {
                    left_has_host: true,
                    right_has_host: false,
                })
            }
        } else {
            Err(HostComparisonError::NoHost {
                left_has_host: false,
                right_has_host: base.has_host(),
            })
        }
    }

    #[test]
    fn has_same_behaviour_than_old_impl() {
        let base1 = "https://www.siemens.com/".parse::<Url>().expect("Success!");
        let base2 = "https://127.0.0.1:8081/whaat"
            .parse::<Url>()
            .expect("Success!");
        let base3 = "file://data/simpen.txt".parse::<Url>().expect("Success!");
        let other1 = "https://www.siemens.com/lookup"
            .parse::<Url>()
            .expect("Success!");
        let other2 = "https://www.google.com/lookup"
            .parse::<Url>()
            .expect("Success!");
        let other3 = "https://127.0.0.1:8080/lookup"
            .parse::<Url>()
            .expect("Success!");
        let other4 = "file://data/simpen.txt".parse::<Url>().expect("Success!");
        let other5 = "file://data/readme.md".parse::<Url>().expect("Success!");
        let bases = [base1, base2, base3];
        let others = [other1, other2, other3, other4, other5];
        for base in &bases {
            for other in &others {
                let old = old_impl(base, other);
                let new = AtraUri::from(other.clone()).compare_hosts(&base.clone().into());
                assert_eq!(
                    old, new,
                    "Failed {old:?} {new:?} for base={base}, other={other}"
                );
            }
        }
    }

    #[test]
    fn can_find_fileendings() {
        let uri1: AtraUri = "https://www.siemens.com/path/to/something/data.pdf"
            .parse()
            .unwrap();
        let uri2: AtraUri = "https://www.siemens.com/path/to/something/data.pdf#help"
            .parse()
            .unwrap();
        let uri3: AtraUri = "https://www.siemens.com/path/to/something/other_data.tar.gz#help"
            .parse()
            .unwrap();
        assert_eq!(Some(vec!["pdf"]), uri1.get_file_endings());
        assert_eq!(Some(vec!["pdf"]), uri2.get_file_endings());
        assert_eq!(Some(vec!["tar", "gz"]), uri3.get_file_endings());
        assert_eq!(Some("pdf"), uri1.file_extension());
        assert_eq!(Some("pdf"), uri2.file_extension());
        assert_eq!(Some("gz"), uri3.file_extension());
    }
}
