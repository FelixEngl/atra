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

use std::io;
use std::io::{IoSliceMut, Read, Seek, SeekFrom};
use std::marker::PhantomData;

pub struct CursorWithLifeline<'a, T> {
    inner: T,
    _life: PhantomData<&'a mut ()>,
}

impl<'a, T> CursorWithLifeline<'a, T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _life: PhantomData,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<'a, T> Read for CursorWithLifeline<'a, T>
where
    T: Read,
{
    delegate::delegate! {
        to self.inner {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
            fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize>;
            fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize>;
            fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize>;
            fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;
        }
    }
}

impl<'a, T> Seek for CursorWithLifeline<'a, T>
where
    T: Seek,
{
    delegate::delegate! {
        to self.inner {
            fn seek(&mut self, pos: SeekFrom) -> io::Result<u64>;
            fn rewind(&mut self) -> io::Result<()>;
            fn stream_position(&mut self) -> io::Result<u64>;
        }
    }
}
