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

mod case_insensitive_str;
pub mod digest;
pub mod domains;
pub mod dropping;
pub mod header_map_extensions;
pub mod isolang_ext;
mod language_detection;
mod limited_buffer;
pub mod selectors;
pub mod serde_ext;
pub mod utf8;

pub use language_detection::*;

pub use case_insensitive_str::*;

/// Compare two optionals by a function.
#[cfg(test)]
pub fn comp_opt<T, F: FnOnce(T, T) -> bool>(a: Option<T>, b: Option<T>, f: F) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => f(a, b),
        (None, None) => true,
        _ => false,
    }
}
