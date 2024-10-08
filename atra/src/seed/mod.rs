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

mod error;
mod guarded;
mod input;
mod unguarded;

use crate::url::{AtraUrlOrigin, UrlWithDepth};
use cfg_if::cfg_if;

pub use guarded::GuardedSeed;
pub use input::lines::read_seeds;
pub use input::seed_data::SeedDefinition;
pub use unguarded::UnguardedSeed;

cfg_if! {
    if #[cfg(test)] {
        pub use error::SeedCreationError;
    }
}

/// The seed of a crawl task
pub trait BasicSeed {
    /// A reference to the url
    fn url(&self) -> &UrlWithDepth;

    /// A reference to the host
    fn origin(&self) -> &AtraUrlOrigin;

    fn is_original_seed(&self) -> bool;

    /// Creates an unguarded version that can be used for storing.
    #[cfg(test)]
    fn create_unguarded(&self) -> UnguardedSeed;
}
