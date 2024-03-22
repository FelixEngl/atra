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

use std::cmp::min;
use std::fs::File;
use std::io;
use std::io::{Cursor, IoSliceMut, SeekFrom, Read};
use serde::{Deserialize, Serialize};
use crate::core::contexts::Context;
use crate::core::io::paths::DataFilePathBuf;

pub type VecDataHolder = DataHolder<Vec<u8>>;

/// Represents the data downloaded, can be nothing, a vec or a file
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Default, Clone)]
pub enum DataHolder<T> {
    #[default]
    None,
    /// We got some data
    InMemory { data: T },
    /// If we are too big we store it in a separate file on the file system
    ExternalFile { file: DataFilePathBuf }
}

impl<T> DataHolder<T> {
    /// Create some data holder for in memory
    #[inline] pub fn from_in_memory(data: T) -> Self {
        Self::InMemory {data}
    }

    /// Create a data holder when the
    #[inline] pub fn from_external(file: DataFilePathBuf) -> Self {
        Self::ExternalFile { file }
    }

    pub fn as_in_memory(&self) -> Option<&T> {
        match self {
            DataHolder::None => {None}
            DataHolder::InMemory { data, .. } => {Some(data)}
            DataHolder::ExternalFile { .. } => {None}
        }
    }
}



impl DataHolder<Vec<u8>> {
    #[inline] pub fn from_vec(data: Vec<u8>) -> Self {
        Self::from_in_memory(data)
    }
}


impl<T: AsRef<[u8]>> DataHolder<T> {

    pub fn size(&self) -> io::Result<u64> {
        match self {
            DataHolder::None => {Ok(0)}
            DataHolder::InMemory { data } => {Ok(data.as_ref().len() as u64)}
            DataHolder::ExternalFile { file } => {
                let file = File::options().read(true).open(file)?;
                Ok(file.metadata()?.len())
            }
        }
    }

    pub fn cursor(&self, context: &impl Context) -> io::Result<Option<DataHolderCursor<&T>>> {
        match self {
            DataHolder::None => {Ok(None)}
            DataHolder::InMemory { data } => {
                Ok(Some(DataHolderCursor::InMemory{
                    len: data.as_ref().len(),
                    cursor: Cursor::new(data)
                }))
            }
            DataHolder::ExternalFile { file: name } => {
                let file = File::options().read(true).open(context.fs().get_unique_path_for_data_file(name))?;
                let len = file.metadata()?.len();
                Ok(Some(DataHolderCursor::FileSystem {
                    len,
                    cursor: file
                }))
            }
        }
    }

    pub fn peek_bom(&self, context: &impl Context) -> io::Result<[u8; 3]> {
        let mut peek = [0u8; 3];
        match self {
            DataHolder::None => {Ok(peek)}
            DataHolder::InMemory { data } => {
                let data = data.as_ref();
                let target_copy = min(3, data.len());
                (&mut peek[..target_copy]).copy_from_slice(&data[..target_copy]);
                Ok(peek)
            }
            DataHolder::ExternalFile { file: name } => {
                let mut file = File::options().read(true).open(context.fs().get_unique_path_for_data_file(name))?;
                file.read(&mut peek)?;
                Ok(peek)
            }
        }
    }
}



/// A cursor for navigating over some kind of data
pub enum DataHolderCursor<T: AsRef<[u8]>> {
    InMemory {
        len: usize,
        cursor: Cursor<T>
    },
    FileSystem {
        len: u64,
        cursor: File
    }
}

impl<T: AsRef<[u8]>> DataHolderCursor<T> {
    #[allow(dead_code)]
    pub fn len(&self) -> u64 {
        match self {
            DataHolderCursor::InMemory { len, .. } => { *len as u64 }
            DataHolderCursor::FileSystem { len, .. } => {*len}
        }
    }
}

impl<T: AsRef<[u8]>> io::Seek for DataHolderCursor<T> {
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

impl<T: AsRef<[u8]>> io::Read for DataHolderCursor<T> {
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