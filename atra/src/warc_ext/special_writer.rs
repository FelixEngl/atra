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

use crate::io::errors::ErrorWithPath;
use camino::Utf8PathBuf;
#[cfg(test)]
use mockall::automock;
use std::io::Read;
use warc::header::WarcHeader;
use warc::writer::WarcWriterError;

/// A writer for WARC files
#[cfg_attr(test, automock)]
pub trait SpecialWarcWriter {
    /// Returns the pointer with the current file and position as tuple, may fail is some kind of error occurs.
    fn get_skip_pointer(&self) -> Result<(Utf8PathBuf, u64), WarcWriterError>;

    /// Returns the pointer with the current file and position as tuple,
    /// wherever it is without checks.
    unsafe fn get_skip_pointer_unchecked(&self) -> (Utf8PathBuf, u64);

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
    fn forward_if_filesize(
        &mut self,
        max_bytes_written: usize,
    ) -> Result<Option<Utf8PathBuf>, ErrorWithPath> {
        if max_bytes_written <= self.bytes_written() {
            self.forward().map(|value| Some(value))
        } else {
            Ok(None)
        }
    }

    /// Forwards to the next file.
    /// Returns the path to the finalized file.
    fn forward(&mut self) -> Result<Utf8PathBuf, ErrorWithPath>;
}
