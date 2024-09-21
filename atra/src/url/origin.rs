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

use crate::toolkit::domains::domain_name_raw;
use crate::toolkit::CaseInsensitiveString;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use url::Url;

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
            None => match self.domain() {
                None => self.host_str().map(|value| value.into()),
                Some(value) => Some(value.into()),
            },
            Some(value) => Some(value.into()),
        }
    }
}

/// The origin of a url. Can be a domain or host in a normalized form.
/// The normalized form is basically a lowercase string
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Hash, Default)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AtraUrlOrigin {
    inner: CaseInsensitiveString,
}

impl Deref for AtraUrlOrigin {
    type Target = CaseInsensitiveString;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AsRef<str> for AtraUrlOrigin {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

impl Display for AtraUrlOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl<T> From<T> for AtraUrlOrigin
where
    T: crate::toolkit::ToCaseInsensitive,
{
    #[inline]
    fn from(value: T) -> Self {
        Self {
            inner: value.to_case_insensitive(),
        }
    }
}
