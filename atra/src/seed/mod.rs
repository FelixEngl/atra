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

mod input;
mod error;
mod guarded;
mod unguarded;
mod provider;

pub use input::lines::read_seeds;
pub use input::seed_data::SeedDefinition;
pub use guarded::GuardedSeed;
pub use unguarded::UnguardedSeed;
pub use error::SeedCreationError;
use crate::url::{UrlWithDepth, AtraUrlOrigin};



/// The seed of a crawl task
pub trait BasicSeed {
    /// A reference to the url
    fn url(&self) -> &UrlWithDepth;

    /// A reference to the host
    fn origin(&self) -> &AtraUrlOrigin;
}