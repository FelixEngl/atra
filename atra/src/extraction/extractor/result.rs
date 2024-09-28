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

use crate::extraction::extractor_method::ExtractorMethod;
use crate::extraction::ExtractedLink;
use std::collections::HashSet;

/// The result of an extraction, contains the extracted links as well es the applied extractors.
#[derive(Debug, Default)]
pub struct ExtractorResult {
    pub links: HashSet<ExtractedLink>,
    pub applied_extractors: HashSet<ExtractorMethod>,
}

impl ExtractorResult {
    /// Returns true if the extractor can be applied
    pub fn apply_extractor(&mut self, extractor: ExtractorMethod) -> bool {
        self.applied_extractors.insert(extractor)
    }

    pub fn register_link(&mut self, link: ExtractedLink) -> bool {
        self.links.insert(link)
    }

    /// Returns true of there are no extracted links
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Returns true if there where no extractors applied.
    pub fn no_extractor_applied(&self) -> bool {
        self.applied_extractors.is_empty()
    }

    /// Converts the result to an optional hashset
    pub fn to_optional_links(self) -> Option<HashSet<ExtractedLink>> {
        if self.is_empty() {
            None
        } else {
            Some(self.links)
        }
    }
}
