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

use camino::{Utf8Path, Utf8PathBuf};
use sealed::sealed;
use thiserror::Error;

type Result<T> = std::result::Result<T, ErrorWithPath>;

/// An error with a path
#[derive(Debug, Error)]
#[error("Failed for file '{path}' with:\n{source}")]
pub struct ErrorWithPath {
    path: Utf8PathBuf,
    #[source]
    source: std::io::Error,
}

impl ErrorWithPath {
    #[inline]
    pub fn new(path: Utf8PathBuf, source: std::io::Error) -> Self {
        Self { path, source }
    }
}

/// Helper trait to convert Result-enums to Result-enums with FSAError
#[sealed]
pub trait ToErrorWithPath<T>
where
    Self: Sized,
{
    /// Converts a normal IO error to an error with a path
    fn to_error_with_path<P: AsRef<Utf8Path>>(self, path: P) -> Result<T>;

    // /// Maps a normal IO error to an error with a path
    // fn map_to_error_with_path<P: AsRef<Utf8Path>, F: FnOnce() -> P>(self, path_provider: F) -> Result<T> {
    //     self.to_error_with_path(path_provider())
    // }
}

#[sealed]
impl<T> ToErrorWithPath<T> for std::result::Result<T, std::io::Error> {
    fn to_error_with_path<P: AsRef<Utf8Path>>(self, path: P) -> Result<T> {
        self.map_err(|e| ErrorWithPath::new(path.as_ref().to_path_buf(), e))
    }

    // fn map_to_error_with_path<P: AsRef<Utf8Path>, F: FnOnce() -> P>(self, path_provider: F) -> Result<T> {
    //     self.map_err(|value| ErrorWithPath::new(path_provider().as_ref().to_path_buf(), value))
    // }
}
