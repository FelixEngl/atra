// Copyright 2024. Felix Engl
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

use crate::queue::AgingQueueElement;
use crate::url::UrlWithDepth;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

/// An entry for the url queue.
#[derive(Debug, Deserialize, Serialize)]
pub struct UrlQueueElement<T = UrlWithDepth> {
    /// The distance between this url and the origin.
    pub is_seed: bool,
    /// The age of the url
    pub age: u32,
    /// Marks if the target was is use.
    pub host_was_in_use: bool,
    /// The target
    pub target: T,
}

impl<T> AgingQueueElement for UrlQueueElement<T> {
    fn age_by_one(&mut self) {
        self.age += 1
    }
}

impl<T> UrlQueueElement<T> {
    pub fn new(is_seed: bool, age: u32, host_was_in_use: bool, target: T) -> Self {
        Self {
            is_seed,
            age,
            host_was_in_use,
            target,
        }
    }

    #[cfg(test)]
    pub fn map<R, F>(self, mapping: F) -> UrlQueueElement<R>
    where
        F: FnOnce(T) -> R,
    {
        UrlQueueElement::new(
            self.is_seed,
            self.age,
            self.host_was_in_use,
            mapping(self.target),
        )
    }

    #[cfg(test)]
    pub fn map_or<R, F>(self, mapping: F) -> Option<UrlQueueElement<R>>
    where
        F: FnOnce(T) -> Option<R>,
    {
        Some(UrlQueueElement::new(
            self.is_seed,
            self.age,
            self.host_was_in_use,
            mapping(self.target)?,
        ))
    }

    #[cfg(test)]
    pub fn map_or_err<R, E, F>(self, mapping: F) -> Result<UrlQueueElement<R>, E>
    where
        F: FnOnce(T) -> Result<R, E>,
    {
        Ok(UrlQueueElement::new(
            self.is_seed,
            self.age,
            self.host_was_in_use,
            mapping(self.target)?,
        ))
    }
}

impl<T: Clone> Clone for UrlQueueElement<T> {
    fn clone(&self) -> Self {
        Self {
            is_seed: self.is_seed,
            age: self.age,
            host_was_in_use: self.host_was_in_use,
            target: self.target.clone(),
        }
    }
}

impl<T: Copy> Copy for UrlQueueElement<T> {}

impl<T: Hash> Hash for UrlQueueElement<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_seed.hash(state);
        self.target.hash(state)
    }
}

impl<T: Display> Display for UrlQueueElement<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CrawlElement(is_seed: {}, age: {}, host_was_in_use: {}, target: {})",
            self.is_seed, self.age, self.host_was_in_use, self.target
        )
    }
}

impl<T> AsRef<T> for UrlQueueElement<T> {
    fn as_ref(&self) -> &T {
        &self.target
    }
}

impl<T: PartialEq> PartialEq<UrlQueueElement<T>> for UrlQueueElement<T> {
    fn eq(&self, other: &UrlQueueElement<T>) -> bool {
        self.target.eq(&other.target)
    }
}
