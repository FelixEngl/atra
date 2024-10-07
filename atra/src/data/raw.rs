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

use crate::format::FileContentReader;
use crate::toolkit::CursorWithLifeline;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::fs::File;
use std::io;
use std::io::{Cursor, IoSliceMut, Read, Seek, SeekFrom};

pub type RawVecData = RawData<Vec<u8>>;


impl From<Option<Vec<u8>>> for RawVecData {
    fn from(value: Option<Vec<u8>>) -> Self {
        match value {
            None => {
                Self::None
            }
            Some(value) => {
                Self::from_vec(value)
            }
        }
    }
}

/// Represents the data downloaded, can be nothing, a vec or a file
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Default, Clone)]
pub enum RawData<T> {
    #[default]
    None,
    /// We got some data
    InMemory { data: T },
    /// If we are too big we store it in a separate file on the file system
    ExternalFile { path: Utf8PathBuf },
}

impl<T> RawData<T> {
    /// Create some data holder for in memory
    #[inline]
    pub fn from_in_memory(data: T) -> Self {
        Self::InMemory { data }
    }

    /// Create a data holder when the
    #[inline]
    pub fn from_external(path: Utf8PathBuf) -> Self {
        Self::ExternalFile { path }
    }

    pub fn as_in_memory(&self) -> Option<&T> {
        match self {
            RawData::None => None,
            RawData::InMemory { data, .. } => Some(data),
            RawData::ExternalFile { .. } => None,
        }
    }
}

impl RawData<Vec<u8>> {
    #[inline]
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self::from_in_memory(data)
    }
}

impl<T: AsRef<[u8]>> FileContentReader for RawData<T> {
    type InMemory = T;
    type Error = io::Error;

    #[inline(always)]
    fn len(&mut self) -> Result<u64, Self::Error> {
        self.size()
    }

    #[inline(always)]
    fn can_read(&self) -> bool {
        !matches!(self, RawData::None)
    }

    #[inline(always)]
    fn cursor(
        &mut self,
    ) -> Result<Option<CursorWithLifeline<impl Seek + Read>>, Self::Error> {
        Ok(RawData::cursor(self)?.map(CursorWithLifeline::new))
    }

    #[inline(always)]
    fn as_in_memory(&mut self) -> Option<&T> {
        RawData::as_in_memory(self)
    }
}

impl<T: AsRef<[u8]>> RawData<T> {
    pub fn size(&self) -> io::Result<u64> {
        match self {
            RawData::None => Ok(0),
            RawData::InMemory { data } => Ok(data.as_ref().len() as u64),
            RawData::ExternalFile { path } => {
                let file = File::options().read(true).open(path)?;
                Ok(file.metadata()?.len())
            },
        }
    }

    pub fn cursor(
        &self,
    ) -> io::Result<Option<DataHolderCursor>> {
        match self {
            RawData::None => Ok(None),
            RawData::InMemory { data } => Ok(Some(DataHolderCursor::InMemory {
                len: data.as_ref().len() as u64,
                cursor: Cursor::new(data.as_ref()),
            })),
            RawData::ExternalFile { path } => {
                let file = File::options()
                    .read(true)
                    .open(path)?;
                let len = file.metadata()?.len();
                Ok(Some(DataHolderCursor::FileSystem { len, cursor: file }))
            },
        }
    }

    pub fn peek_bom(&self) -> io::Result<[u8; 3]> {
        let mut peek = [0u8; 3];
        match self {
            RawData::None => Ok(peek),
            RawData::InMemory { data } => {
                let data = data.as_ref();
                let target_copy = min(3, data.len());
                (&mut peek[..target_copy]).copy_from_slice(&data[..target_copy]);
                Ok(peek)
            }
            RawData::ExternalFile { path } => {
                let mut file = File::options()
                    .read(true)
                    .open(path)?;
                file.read(&mut peek)?;
                Ok(peek)
            }
        }
    }
}

/// A cursor for navigating over some kind of data
pub enum DataHolderCursor<'a> {
    InMemory { len: u64, cursor: Cursor<&'a [u8]> },
    FileSystem { len: u64, cursor: File },
}

impl<'a> DataHolderCursor<'a> {
    pub fn len(&self) -> u64 {
        match self {
            DataHolderCursor::InMemory { len, .. } => *len,
            DataHolderCursor::FileSystem { len, .. } => *len,
        }
    }
}

impl<'a> io::Seek for DataHolderCursor<'a> {
    delegate::delegate! {
        to match self {
            Self::InMemory{cursor, ..} => cursor,
            Self::FileSystem{cursor, ..} => cursor,
        } {
            fn seek(&mut self, pos: SeekFrom) -> io::Result<u64>;
            fn rewind(&mut self) -> io::Result<()>;
            fn stream_position(&mut self) -> io::Result<u64>;
        }
    }
}

impl<'a> Read for DataHolderCursor<'a> {
    delegate::delegate! {
        to match self {
            Self::InMemory{cursor, ..} => cursor,
            Self::FileSystem{cursor, ..} => cursor,
        } {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
            fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize>;
            fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize>;
            fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize>;
            fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;
        }
    }
}
