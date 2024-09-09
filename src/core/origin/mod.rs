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

pub mod manager;
pub mod guard;
pub mod errors;
pub mod entry;
pub mod managers;


pub use manager::OriginManager;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use url::Url;
use crate::core::crawl::website_crawler::domain_name_raw;

/// Provides the origin to something
pub trait AtraOriginProvider {
    /// Returns an origin if one exists
    fn atra_origin(&self) -> Option<AtraUrlOrigin>;
}

impl AtraOriginProvider for Url {
    /// Returns the domain or host as string.
    /// Tries to get the best descriptive string for the origin.
    /// Prefers domain to host. The case of the value is standardized by the type of address.
    /// e.g. For URLs the case is irrelevant, hence lower case is used.
    fn atra_origin(&self) -> Option<AtraUrlOrigin> {
        match domain_name_raw(self) {
            None => {
                match self.domain() {
                    None => {
                        self.host_str().map(|value| value.to_lowercase().into())
                    }
                    Some(value) => {
                        Some(value.to_lowercase().into())
                    }
                }
            }
            Some(value) => {
                Some(AtraUrlOrigin::from(value.as_bytes()))
            }
        }
    }
}


/// The origin of a url. Can be a domain or host in a normalized form.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Hash, Default)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AtraUrlOrigin(String);

impl AsRef<str> for AtraUrlOrigin {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for AtraUrlOrigin {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl<'a> From<&'a str> for AtraUrlOrigin {
    fn from(value: &'a str) -> Self {
        Self(value.to_owned())
    }
}

impl<'a> From<&'a [u8]> for AtraUrlOrigin {
    fn from(value: &'a [u8]) -> Self {
        Self(String::from_utf8_lossy(value).into_owned())
    }
}

impl AtraUrlOrigin {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl FromStr for AtraUrlOrigin {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl Display for AtraUrlOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}