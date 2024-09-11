use std::io::{ErrorKind, IoSliceMut, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ByteCounterReaderWrapper<R> {
    inner: R,
    counter: Arc<AtomicU64>
}

impl<R> ByteCounterReaderWrapper<R> {
    pub fn new(inner: R, counter: Arc<AtomicU64>) -> Self {
        Self { inner, counter }
    }

    pub fn wrap(inner: R) -> (Self, Arc<AtomicU64>) {
        let counter: Arc<AtomicU64> = Default::default();
        (Self::new(inner, counter.clone()), counter)
    }

    pub fn get_counter_view(&self) -> Arc<AtomicU64> {
        self.counter.clone()
    }

    #[inline(always)]
    fn add_usize_result(&self, result: std::io::Result<usize>) -> std::io::Result<usize>{
        match result {
            Ok(value) => {
                self.counter.fetch_add(value as u64, Ordering::Release);
                Ok(value)
            }
            x @ Err(_) => x
        }
    }
}

impl<R> Read for ByteCounterReaderWrapper<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = self.inner.read(buf);
        self.add_usize_result(result)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
        let result = self.inner.read_vectored(bufs);
        self.add_usize_result(result)
    }

    #[cfg(RUSTC_IS_NIGHTLY)]
    #[feature(can_vector)]
    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let result = self.inner.read_to_end(buf);
        self.add_usize_result(result)
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        let result = self.inner.read_to_string(buf);
        self.add_usize_result(result)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        match self.inner.read_exact(buf) {
            Ok(_) => {
                self.counter.fetch_add(buf.len() as u64, Ordering::Release);
                Ok(())
            }
            Err(err) => {
                if err.kind() == ErrorKind::Interrupted {
                    self.counter.fetch_add(buf.len() as u64, Ordering::Release);
                }
                Err(err)
            }
        }
    }

    #[cfg(RUSTC_IS_NIGHTLY)]
    #[feature(read_buf)]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> std::io::Result<()> {
        let written = buf.written();
        let result = self.inner.read_buf(buf);
        self.counter.fetch_add((buf.written() - written) as u64, Ordering::Release);
        result
    }

    #[cfg(RUSTC_IS_NIGHTLY)]
    #[feature(read_buf)]
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> std::io::Result<()> {
        let written = cursor.written();
        let result = self.inner.read_buf_exact(cursor);
        self.counter.fetch_add((cursor.written() - written) as u64, Ordering::Release);
        result
    }
}


