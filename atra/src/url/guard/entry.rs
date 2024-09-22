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

use crate::url::Depth;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// The entry for an origin
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct GuardEntry {
    pub(super) is_in_use: bool,
    pub(super) last_modification: Option<SystemTime>,
    pub(super) depth: Depth,
}

impl GuardEntry {
    /// Returns true if the guarded entry is in use.
    pub fn is_in_use(&self) -> bool {
        self.is_in_use
    }

    /// Returns the last modification timestamp
    pub fn last_modification(&self) -> Option<SystemTime> {
        self.last_modification
    }

    /// The depth of the protected domain.
    pub fn depth(&self) -> Depth {
        self.depth
    }
}
