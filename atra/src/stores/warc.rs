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

use crate::io::errors::{ErrorWithPath, ToErrorWithPath};
use crate::io::file_owner::FileOwner;
use crate::io::fs::WorkerFileSystemAccess;
use crate::warc_ext::SpecialWarcWriter;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::sync::Arc;
use tokio::sync::RwLock;
use warc::header::WarcHeader;
use warc::writer::{WarcWriter, WarcWriterError};

pub trait WarcFilePathProvider {
    /// Creates a fresh warc file
    fn create_new_warc_file_path(&self) -> Result<Utf8PathBuf, ErrorWithPath>;
}

pub trait RawWriter: Write {
    fn create_for_warc(path: impl AsRef<Utf8Path>) -> Result<Self, ErrorWithPath>
    where
        Self: Sized;
}
impl RawWriter for File {
    fn create_for_warc(path: impl AsRef<Utf8Path>) -> Result<Self, ErrorWithPath> {
        let result = path.as_ref();
        File::options()
            .write(true)
            .create_new(true)
            .open(result)
            .to_error_with_path(result)
    }
}

#[derive(Debug)]
pub struct ThreadsafeMultiFileWarcWriter<
    W: Write + RawWriter = File,
    P: WarcFilePathProvider = WorkerFileSystemAccess,
> {
    writer: Arc<RwLock<RawMultifileWarcWriter<W, P>>>,
}

impl ThreadsafeMultiFileWarcWriter<File, WorkerFileSystemAccess> {
    pub fn new_for_worker(fp: Arc<WorkerFileSystemAccess>) -> Result<Self, ErrorWithPath> {
        Self::try_from(fp)
    }
}

impl<W: Write + RawWriter, P: WarcFilePathProvider> TryFrom<Arc<P>>
    for ThreadsafeMultiFileWarcWriter<W, P>
{
    type Error = ErrorWithPath;

    fn try_from(value: Arc<P>) -> Result<Self, Self::Error> {
        let path = value.create_new_warc_file_path()?;
        let writer = W::create_for_warc(&path)?;
        Ok(Self {
            writer: Arc::new(RwLock::new(RawMultifileWarcWriter::new(
                value,
                WarcWriter::new(BufWriter::new(writer)),
                path,
            ))),
        })
    }
}

#[allow(dead_code)]
impl<W: Write + RawWriter, P: WarcFilePathProvider> ThreadsafeMultiFileWarcWriter<W, P> {
    pub fn new(writer: W, provider: P, path: Utf8PathBuf) -> Self {
        Self {
            writer: Arc::new(RwLock::new(RawMultifileWarcWriter::new(
                Arc::new(provider),
                WarcWriter::new(BufWriter::new(writer)),
                path,
            ))),
        }
    }

    pub async fn current_file(&self) -> Utf8PathBuf {
        let writer = self.writer.read().await;
        writer.path.clone()
    }

    pub async fn flush(&self) -> Result<(), ErrorWithPath> {
        let mut writer = self.writer.write().await;
        writer.flush()
    }

    pub async fn execute_on_writer<
        R,
        E,
        F: FnOnce(&mut RawMultifileWarcWriter<W, P>) -> Result<R, E>,
    >(
        &self,
        to_execute: F,
    ) -> Result<R, E> {
        log::trace!("Get WARC-Write lock");
        let mut writer = self.writer.write().await;
        log::trace!("Get WARC-Write lock - success");
        to_execute(&mut writer)
    }
}

impl<W: Write + RawWriter, P: WarcFilePathProvider> FileOwner
    for ThreadsafeMultiFileWarcWriter<W, P>
{
    fn is_in_use<Q: AsRef<Utf8Path>>(&self, path: Q) -> bool {
        match self.writer.try_read() {
            Ok(value) => value.path.as_path() == path.as_ref(),
            Err(_) => true,
        }
    }

    async fn wait_until_free_path<Q: AsRef<Utf8Path>>(
        &self,
        target: Q,
    ) -> Result<(), ErrorWithPath> {
        let path = target.as_ref();
        let writer = self.writer.read().await;
        if writer.path.as_path() == path {
            drop(writer);
            let mut writer = self.writer.write().await;
            let _ = writer.forward_if_filesize(0)?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl<W: Write + RawWriter, P: WarcFilePathProvider> Clone for ThreadsafeMultiFileWarcWriter<W, P> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
        }
    }
}

#[derive(Debug)]
pub struct RawMultifileWarcWriter<W: Write + RawWriter, P: WarcFilePathProvider> {
    fp: Arc<P>,
    writer: WarcWriter<BufWriter<W>>,
    path: Utf8PathBuf,
}

#[allow(dead_code)]
impl<W: Write + RawWriter, P: WarcFilePathProvider> RawMultifileWarcWriter<W, P> {
    pub fn new(fp: Arc<P>, writer: WarcWriter<BufWriter<W>>, path: Utf8PathBuf) -> Self {
        Self { fp, writer, path }
    }

    fn flush(&mut self) -> Result<(), ErrorWithPath> {
        self.writer.flush().to_error_with_path(&self.path)
    }

    fn replace_writer(
        &mut self,
        writer: WarcWriter<BufWriter<W>>,
        path: Utf8PathBuf,
    ) -> (WarcWriter<BufWriter<W>>, Utf8PathBuf) {
        (
            std::mem::replace(&mut self.writer, writer),
            std::mem::replace(&mut self.path, path),
        )
    }
}

impl<W: Write + RawWriter, P: WarcFilePathProvider> SpecialWarcWriter
    for RawMultifileWarcWriter<W, P>
{
    fn get_skip_pointer(&self) -> Result<(Utf8PathBuf, u64), WarcWriterError> {
        self.writer
            .check_if_state(warc::states::State::ExpectHeader)?;
        Ok((self.path.clone(), self.writer.bytes_written() as u64))
    }

    unsafe fn get_skip_pointer_unchecked(&self) -> (Utf8PathBuf, u64) {
        (self.path.clone(), self.writer.bytes_written() as u64)
    }

    #[inline]
    fn bytes_written(&self) -> usize {
        self.writer.bytes_written()
    }

    #[inline]
    fn write_header(&mut self, header: WarcHeader) -> Result<usize, WarcWriterError> {
        self.writer.write_header(&header)
    }

    #[inline]
    fn write_body_complete(&mut self, buf: &[u8]) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(buf)
    }

    #[inline]
    fn write_body<R: Read>(&mut self, body: &mut R) -> Result<usize, WarcWriterError> {
        self.writer.write_body(body)
    }

    #[inline]
    fn write_empty_body(&mut self) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(&[])
    }

    fn forward(&mut self) -> Result<Utf8PathBuf, ErrorWithPath> {
        let path = self.fp.create_new_warc_file_path()?;
        let (mut old_writer, path) = self.replace_writer(
            WarcWriter::new(BufWriter::new(W::create_for_warc(&path)?)),
            path,
        );
        old_writer.flush().to_error_with_path(&path)?;
        Ok(path)
    }
}
