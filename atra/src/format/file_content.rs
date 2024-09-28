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

use std::error::Error;
use std::io::{Read, Seek};
use std::sync::OnceLock;

use zip::result::ZipError;
use zip::ZipArchive;

use crate::toolkit::CursorWithLifeline;

/// A trait exposing the minimum of a file content.
/// Even if this trait allows mutable access it has to be guaranteed,
/// that the underlying data is never changed.
pub trait FileContentReader {
    type InMemory: AsRef<[u8]> + Sized;

    type Error: Error;

    fn len(&mut self) -> Result<u64, Self::Error>;

    /// Returns true if it can cal cursor without return ing a None
    fn can_read(&self) -> bool;

    #[allow(clippy::needless_lifetimes)]
    /// Get a cursor like object to read the data
    fn cursor<'a>(
        &'a mut self,
    ) -> Result<Option<CursorWithLifeline<'a, impl Seek + Read>>, Self::Error>;

    /// Returns the in memory representation is possible.
    fn as_in_memory(&mut self) -> Option<&Self::InMemory>;
}

pub struct ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    archive: &'a mut ZipArchive<R>,
    file_idx: usize,
    max_in_memory: u64,
    cached_content: OnceLock<Option<Vec<u8>>>,
}

impl<'a, R> ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    pub fn new(
        archive: &'a mut ZipArchive<R>,
        file_idx: usize,
        max_in_memory: usize,
        cached_content: Option<Vec<u8>>,
    ) -> Self {
        let cached_content = if let Some(cached_content) = cached_content {
            OnceLock::from(Some(cached_content))
        } else {
            OnceLock::new()
        };

        Self {
            archive,
            file_idx,
            max_in_memory: max_in_memory as u64,
            cached_content,
        }
    }

    pub fn file_name(&mut self) -> Result<Option<String>, ZipError> {
        let archive = self.archive.by_index(self.file_idx)?;
        if archive.is_file() {
            let file_name = archive
                .enclosed_name()
                .map(|path| {
                    path.file_name()
                        .map(|name| name.to_os_string().into_string().ok())
                        .flatten()
                })
                .flatten()
                .unwrap_or_else(|| {
                    if let Some((_, last_name)) = archive.name().rsplit_once('/') {
                        last_name.to_string()
                    } else {
                        archive.name().to_string()
                    }
                });
            Ok(Some(file_name))
        } else {
            Ok(None)
        }
    }

    pub fn file_name_and_len(&mut self) -> Result<Option<(String, u64)>, ZipError> {
        let a = self.file_name()?;
        match a {
            None => Ok(None),
            Some(value) => Ok(Some((value, self.len()?))),
        }
    }

    pub fn zip_reader(&mut self) -> &mut ZipArchive<R> {
        self.archive
    }
}

impl<'a, R> FileContentReader for ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    type InMemory = Vec<u8>;
    type Error = ZipError;

    fn len(&mut self) -> Result<u64, Self::Error> {
        Ok(self.archive.by_index(self.file_idx)?.size())
    }

    #[inline(always)]
    fn can_read(&self) -> bool {
        true
    }

    fn cursor(
        &mut self,
    ) -> Result<Option<CursorWithLifeline<impl Seek + Read>>, Self::Error> {
        let seeker = self.archive.by_index_seek(self.file_idx)?;
        let cursor = CursorWithLifeline::new(seeker);
        Ok(Some(cursor))
    }

    fn as_in_memory(&mut self) -> Option<&Self::InMemory> {
        let mut found = self.archive.by_index(self.file_idx).ok()?;
        if found.size() <= self.max_in_memory {
            self.cached_content
                .get_or_init(|| {
                    if found.size() == 0 {
                        None
                    } else {
                        let mut value = Vec::with_capacity(found.size() as usize);
                        found.read_to_end(&mut value).ok().and(Some(value))
                    }
                })
                .as_ref()
        } else {
            None
        }
    }
}
