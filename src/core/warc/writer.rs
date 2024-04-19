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

use std::io::Read;
use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use crate::core::io::fs::FSAError;
use crate::core::io::paths::DataFilePathBuf;
use crate::warc::header::{WarcHeader};
use crate::warc::writer::WarcWriterError;
#[cfg(test)]
use mockall::{automock};

/// A pointer to the start of an entry in a warc [file]
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WarcSkipPointer {
    file: DataFilePathBuf,
    position: u64
}

impl WarcSkipPointer {
    pub fn new(
        path: DataFilePathBuf,
        position: u64
    ) -> Self {
        Self {
            file: path,
            position
        }
    }

    /// Offset from the start of the file to the start of the WARC-Header
    #[allow(dead_code)]
    #[inline] pub fn position(&self) -> u64 {
        self.position
    }

    /// The file with the associated WARC entry
    #[allow(dead_code)]
    #[inline] pub fn file(&self) -> &Utf8Path {
        &self.file
    }
}

/// A skip pointer with additional informations
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WarcSkipPointerWithOffsets {
    skip: WarcSkipPointer,
    warc_header_offset: u32,
    body_octet_count: u64
}

impl WarcSkipPointerWithOffsets {
    pub fn new(skip: WarcSkipPointer, warc_header_offset: u32, body_octet_count: u64) -> Self {
        Self {
            skip,
            warc_header_offset,
            body_octet_count
        }
    }

    /// Offset from the start of the file to the start of the WARC-Header
    #[inline] pub fn position(&self) -> u64 {
        self.skip.position
    }

    /// The file with the associated WARC entry
    #[inline] pub fn file(&self) -> &Utf8Path {
        &self.skip.file
    }

    /// The size of the warc header in bytes
    #[inline] pub fn warc_header_offset(&self) -> u32 {self.warc_header_offset }

    /// The number of octets in the whole body
    #[inline] pub fn body_octet_count(&self) -> u64 {self.body_octet_count}
}

/// A writer for WARC files
#[cfg_attr(test, automock)]
pub trait SpecialWarcWriter {
    /// Returns the pointer, may fail is some kind of error occurs.
    fn get_skip_pointer(&self) -> Result<WarcSkipPointer, WarcWriterError>;

    /// Returns the pointer wherever it is without checks.
    unsafe fn get_skip_pointer_unchecked(&self) -> WarcSkipPointer;

    /// Returns the number of bytes written to the file
    fn bytes_written(&self) -> usize;

    /// Writes a warc header to the file.
    /// Returns the number of bytes written.
    fn write_header(&mut self, header: WarcHeader) -> Result<usize, WarcWriterError>;

    /// Writes a body to the file
    /// Returns the number of bytes written. (including the tail)
    fn write_body_complete(&mut self, buf: &[u8]) -> Result<usize, WarcWriterError>;


    /// Writes a body to the file
    /// Returns the number of bytes written. (including the tail)
    #[cfg_attr(test, mockall::concretize)]
    fn write_body<R: Read>(&mut self, body: &mut R) -> Result<usize, WarcWriterError>;

    /// Writes an empty body to the file
    /// Returns the number of bytes written. (including the tail)
    fn write_empty_body(&mut self) -> Result<usize, WarcWriterError>;

    /// Forwards to the next file, iff the number of bytes written is greater than [max_bytes_written]
    /// Returns the path to the finalized file.
    fn forward_if_filesize(&mut self, max_bytes_written: usize) -> Result<Option<DataFilePathBuf>, FSAError> {
        if max_bytes_written <= self.bytes_written() {
            self.forward().map(|value| Some(value))
        } else {
            Ok(None)
        }
    }

    /// Forwards to the next file.
    /// Returns the path to the finalized file.
    fn forward(&mut self) -> Result<DataFilePathBuf, FSAError>;
}


