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
use std::io::{Bytes, Chain, IoSliceMut, Read, Seek, SeekFrom, Take};
use std::sync::OnceLock;
use zip::read::{ZipFileSeek};
use zip::result::ZipError;
use zip::ZipArchive;
use crate::contexts::traits::SupportsFileSystemAccess;

/// A trait exposing the minimum of a file content.
pub trait FileContent {
    type CursorLike: Read + Seek;

    type InMemory: AsRef<[u8]> + Sized;

    type Error: Error;

    /// Get a cursor like object to read the data
    fn cursor(&self, context: &impl SupportsFileSystemAccess) -> Result<Option<Self::CursorLike>, Self::Error>;

    /// Returns the in memory representation is possible.
    fn as_in_memory(&self) -> Option<&Self::InMemory>;
}


pub struct ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    archive: ZipArchive<R>,
    file_name: &'a str,
    max_in_memory: u64,
    cached_content: OnceLock<Option<Vec<u8>>>,
}

impl<'a, R> ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    pub fn new(archive: ZipArchive<R>, file_name: &'a str, max_in_memory: usize, cached_content: Option<Vec<u8>>) -> Self {
        let cached_content = if let Some(cached_content) = cached_content {
            OnceLock::from(Some(cached_content))
        } else {
            OnceLock::new()
        };

        Self {
            archive,
            file_name,
            max_in_memory: max_in_memory as u64,
            cached_content
        }
    }
}

impl<'a, R> FileContent for ZipFileContent<'a, R>
where
    R: Seek + Read,
{
    type CursorLike = ZipFileContentCursor<R>;
    type InMemory = Vec<u8>;
    type Error = ZipError;

    fn cursor(&self, _: &impl SupportsFileSystemAccess) -> Result<Option<Self::CursorLike>, Self::Error> {
        let mut archive = self.archive.clone();
        let seeker = archive.by_name_seek(self.file_name)?;
        let seeker: ZipFileSeek<'static, R> = unsafe { std::mem::transmute(seeker) };
        Ok(Some(ZipFileContentCursor {
            archive,
            seeker,
        }))
    }

    fn as_in_memory(&self) -> Option<&Self::InMemory> {
        let mut archive = self.archive.clone();
        let mut found = archive.by_name(self.file_name).ok()?;
        if found.size() <= self.max_in_memory {
            self.cached_content.get_or_init(|value| {
                if found.size() == 0 {
                    None
                } else {
                    let mut value = Vec::with_capacity(found.size() as  usize);
                    found
                        .read_to_end(&mut value)
                        .ok()
                        .and(Some(value))
                }
            }).as_ref()
        } else {
            None
        }
    }
}

pub struct ZipFileContentCursor<R> {
    archive: ZipArchive<R>,
    seeker: ZipFileSeek<'static, R>,
}

impl<R> Seek for ZipFileContentCursor<R> where R:Seek {
    delegate::delegate! {
        to self.seeker {
            fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64>;
            fn rewind(&mut self) -> std::io::Result<()>;
            fn stream_position(&mut self) -> std::io::Result<u64>;
            fn seek_relative(&mut self, offset: i64) -> std::io::Result<()>;
        }
    }
}

impl<R> Read for ZipFileContentCursor<R> where R: Read {
    delegate::delegate! {
        to self.seeker {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
            fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize>;
            fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize>;
            fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize>;
            fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()>;
            fn bytes(self) -> Bytes<Self> where Self: Sized;
            fn chain<R: Read>(self, next: R) -> Chain<Self, R> where Self: Sized;
            fn take(self, limit: u64) -> Take<Self> where Self: Sized;

        }
    }
}