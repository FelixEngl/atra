use std::fs::File;
use std::io::{BufWriter, Read};
use std::sync::Arc;
use camino::Utf8Path;
use tokio::sync::RwLock;
use crate::core::io::fs::{FSAError, ToFSAError, WorkerFileProvider};
use crate::core::io::paths::DataFilePathBuf;
use crate::core::io::templating::FileNameTemplate;
use crate::core::warc::SpecialWarcWriter;
use crate::core::warc::writer::WarcSkipPointer;
use crate::warc::header::WarcHeader;
use crate::warc::writer::{WarcWriter, WarcWriterError};

#[derive(Debug)]
pub struct ThreadsafeWarcWriter {
    template: FileNameTemplate,
    writer: Arc<RwLock<RawWorkerWarcWriter>>
}

impl ThreadsafeWarcWriter {
    pub fn new(template: FileNameTemplate, fp: Arc<WorkerFileProvider>) -> Result<Self, FSAError> {
        let (file, path) = fp.create_fresh_warc_file(None::<&str>)?;
        Ok(
            Self {
                template,
                writer: Arc::new(RwLock::new(RawWorkerWarcWriter::new(
                    fp,
                    WarcWriter::new(BufWriter::new(file)),
                    DataFilePathBuf::new(path)
                ))),
            }
        )
    }

    #[allow(dead_code)]
    pub async fn current_file(&self) -> DataFilePathBuf {
        let writer = self.writer.read().await;
        writer.path.clone()
    }

    #[allow(dead_code)]
    pub async fn flush(&self) -> Result<(), FSAError> {
        let mut writer = self.writer.write().await;
        writer.flush()
    }

    pub async fn execute_on_writer<R, E, F: FnOnce(&mut RawWorkerWarcWriter) -> Result<R, E>>(&self, to_execute: F) -> Result<R, E> {
        log::trace!("Get WARC-Write lock");
        let mut writer = self.writer.write().await;
        log::trace!("Get WARC-Write lock - success");
        to_execute(&mut writer)
    }

    pub async fn make_sure_is_not_write_target(&self, path: &Utf8Path) -> Result<(), FSAError> {
        let writer = self.writer.read().await;
        if writer.path.as_path().eq(path) {
            drop(writer);
            let mut writer = self.writer.write().await;
            let _ = writer.forward_if_filesize(0)?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Clone for ThreadsafeWarcWriter {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            template: self.template.clone()
        }
    }
}


#[derive(Debug)]
pub struct RawWorkerWarcWriter {
    fp: Arc<WorkerFileProvider>,
    writer: WarcWriter<BufWriter<File>>,
    path: DataFilePathBuf
}

impl RawWorkerWarcWriter {

    pub fn new(
        fp: Arc<WorkerFileProvider>,
        writer: WarcWriter<BufWriter<File>>,
        path: DataFilePathBuf
    ) -> Self {
        Self {
            fp,
            writer,
            path
        }
    }

    #[allow(dead_code)]
    fn flush(&mut self) -> Result<(), FSAError> {
        self.writer.flush().to_fsa_error(|| self.path.to_string())
    }

    fn replace_writer(&mut self, writer: WarcWriter<BufWriter<File>>, path: DataFilePathBuf) -> (WarcWriter<BufWriter<File>>, DataFilePathBuf) {
        (std::mem::replace(&mut self.writer, writer), std::mem::replace(&mut self.path, path))
    }
}

impl SpecialWarcWriter for RawWorkerWarcWriter {
    fn get_skip_pointer(&self) -> Result<WarcSkipPointer, WarcWriterError> {
        self.writer.check_if_state(crate::warc::states::State::ExpectHeader)?;
        Ok(
            WarcSkipPointer::new(
                self.path.clone(),
                self.writer.bytes_written() as u64
            )
        )
    }

    unsafe fn get_skip_pointer_unchecked(&self) -> WarcSkipPointer {
        WarcSkipPointer::new(
            self.path.clone(),
            self.writer.bytes_written() as u64
        )
    }


    #[inline] fn bytes_written(&self) -> usize {
        self.writer.bytes_written()
    }

    #[inline] fn write_header(&mut self, header: WarcHeader) -> Result<usize, WarcWriterError> {
        self.writer.write_header(&header)
    }

    #[inline] fn write_body_complete(&mut self, buf: &[u8]) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(buf)
    }


    #[inline] fn write_body<R: Read>(&mut self, body: &mut R) -> Result<usize, WarcWriterError> {
        self.writer.write_body(body)
    }

    #[inline] fn write_empty_body(&mut self) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(&[])
    }

    fn forward(&mut self) -> Result<DataFilePathBuf, FSAError> {
        let (file, path) = self.fp.create_fresh_warc_file(None::<&str>)?;
        let (mut old_writer, path) = self.replace_writer(
            WarcWriter::new(BufWriter::new(file)),
            DataFilePathBuf::new(path)
        );
        old_writer.flush().map_err(|value| FSAError(path.as_path().to_string(), value))?;
        Ok(path)
    }
}
