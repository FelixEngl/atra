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

use crate::extraction::extractor::{ApplyWhen};
use crate::extraction::extractor_method::ExtractorMethod;
use crate::format::AtraFileInformation;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq)]
pub struct ExtractorCommand {
    pub extractor_method: ExtractorMethod,
    pub apply_when: ApplyWhen,
}

impl Display for ExtractorCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.extractor_method, self.apply_when)
    }
}

impl ExtractorCommand {
    pub fn new(extractor_method: ExtractorMethod, apply_when: ApplyWhen) -> Self {
        Self {
            extractor_method,
            apply_when,
        }
    }

    pub fn new_default_apply(extractor_method: ExtractorMethod) -> Self {
        match &extractor_method {
            ExtractorMethod::BinaryHeuristic => Self::new(extractor_method, ApplyWhen::Fallback),
            _ => Self::new(extractor_method, Default::default()),
        }
    }

    pub fn can_apply(&self, file_info: &AtraFileInformation) -> bool {
        match self.apply_when {
            ApplyWhen::Always => true,
            ApplyWhen::IfSuitable => self.extractor_method.is_compatible(file_info),
            ApplyWhen::Fallback => false,
        }
    }

    pub fn can_extract(&self, file_info: &AtraFileInformation) -> bool {
        self.extractor_method.is_compatible(file_info)
    }

    pub fn is_fallback(&self) -> bool {
        return self.apply_when == ApplyWhen::Fallback;
    }
}

impl AsRef<ApplyWhen> for ExtractorCommand {
    fn as_ref(&self) -> &ApplyWhen {
        &self.apply_when
    }
}

impl PartialEq<Self> for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn eq(&self, #[as_ref] other: &Self) -> bool;
        }
    }
}

impl PartialOrd<Self> for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn partial_cmp(&self, #[as_ref] other: &Self) -> Option<Ordering>;
        }
    }
}

impl Ord for ExtractorCommand {
    delegate::delegate! {
        to self.apply_when {
            fn cmp(&self, #[as_ref] other: &Self) -> Ordering;
        }
    }
}
