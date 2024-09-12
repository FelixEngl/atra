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

use crate::header::{WarcHeader, WarcHeaderWriteError};
use crate::states::State;
use std::fmt::{Debug, Formatter};
use std::io;
use std::io::{Read, Write};
use thiserror::Error;
use ubyte::ByteUnit;

/// A writer for a warc file writes the content according to the values
/// warc-file    = 1*warc-record
/// warc-record  = header CRLF
///                block CRLF CRLF
/// header       = version warc-fields
/// version      = "WARC/1.1" CRLF
/// warc-fields  = *named-field CRLF
/// block        = *OCTET
pub struct WarcWriter<W: Write> {
    inner: W,
    bytes_written: usize,
    state: State,
    corrupt: bool,
}

const BODY_TAIL: &[u8; 4] = b"\r\n\r\n";

impl<W: Write + Debug> Debug for WarcWriter<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarcWriter")
            .field("inner", &self.inner)
            .field("bytes_written", &self.bytes_written)
            .field("state", &self.state)
            .finish()
    }
}

/// The errors of the writer
#[derive(Debug, Error)]
pub enum WarcWriterError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error("Current state is {current} but expected {expected}!")]
    WrongStateError { current: State, expected: State },
    #[error("The writer is corrupted.")]
    Corrupt,
    #[error(transparent)]
    HeaderError(#[from] WarcHeaderWriteError),
}

impl<W: Write> WarcWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            bytes_written: 0,
            state: State::ExpectHeader,
            corrupt: false,
        }
    }

    /// The number of bytes written
    pub fn bytes_written(&self) -> usize {
        self.bytes_written
    }

    /// Returns an error if the state is not the [expected].
    pub fn check_if_state(&self, expected: State) -> Result<(), WarcWriterError> {
        if self.corrupt {
            return Err(WarcWriterError::Corrupt);
        }
        if self.state != expected {
            Err(WarcWriterError::WrongStateError {
                current: self.state,
                expected,
            })
        } else {
            Ok(())
        }
    }

    /// The current state

    pub fn state(&self) -> State {
        self.state
    }

    /// Returns true if the writer failed somewhere in a non recoverable way.
    pub fn corrupted(&self) -> bool {
        self.corrupt
    }

    /// Sets the corruption flag to [new_corrupt].
    /// May cause the production of illegal WARC archives.
    pub unsafe fn set_corrupt(&mut self, new_corrupt: bool) -> bool {
        std::mem::replace(&mut self.corrupt, new_corrupt)
    }

    /// Sets the state to [new_state].
    /// May cause the production of illegal WARC archives.
    pub unsafe fn set_state(&mut self, new_state: State) -> State {
        std::mem::replace(&mut self.state, new_state)
    }

    /// Writes a [header] to the file.
    /// Excpects after that a body.
    /// Returns the number of bytes written (including the following newlines)
    pub fn write_header(&mut self, header: &WarcHeader) -> Result<usize, WarcWriterError> {
        self.check_if_state(State::ExpectHeader)?;
        let written = match header.write_to(&mut self.inner, true) {
            Ok(value) => value,
            Err(err) => {
                self.corrupt = true;
                return Err(err.into());
            }
        };
        self.bytes_written += written;
        self.state = State::ExpectBody;
        return Ok(written);
    }

    /// Write the body tail, does not increment the bytes written.
    fn write_body_tail(&mut self) -> Result<(), WarcWriterError> {
        match self.inner.write(BODY_TAIL) {
            Ok(_) => Ok(()),
            Err(err) => {
                self.corrupt = true;
                Err(err.into())
            }
        }
    }

    /// Writes a [body] to the file.
    /// Excpects after that a header.
    /// Returns the number of bytes written (including the following newlines)
    pub fn write_complete_body(&mut self, body: &[u8]) -> Result<usize, WarcWriterError> {
        self.check_if_state(State::ExpectBody)?;
        if !body.is_empty() {
            match self.inner.write_all(body) {
                Ok(_) => {}
                Err(err) => {
                    self.corrupt = true;
                    return Err(err.into());
                }
            }
            self.bytes_written += body.len();
        }
        self.write_body_tail()?;
        self.bytes_written += 4;
        self.state = State::ExpectHeader;
        Ok(body.len() + 4)
    }

    /// Writes a [body] to the file.
    /// Excpects after that a header.
    /// Returns the number of bytes written (including the following newlines)
    pub fn write_body(&mut self, body: &mut impl Read) -> Result<usize, WarcWriterError> {
        self.check_if_state(State::ExpectBody)?;
        let mut buffer = [0u8; ByteUnit::Megabyte(1).as_u64() as usize];
        let mut bytes_written = 0usize;
        loop {
            let read = match body.read(&mut buffer) {
                Ok(value) => value,
                Err(err) => {
                    if bytes_written > 0 {
                        self.corrupt = true
                    }
                    return Err(err.into());
                }
            };
            if read == 0 {
                break;
            }
            match self.inner.write(&buffer[..read]) {
                Ok(written) => {
                    self.bytes_written += written;
                    bytes_written += written;
                }
                Err(err) => {
                    self.corrupt = true;
                    return Err(err.into());
                }
            }
        }
        self.write_body_tail()?;
        self.bytes_written += 4;
        self.state = State::ExpectHeader;
        Ok(bytes_written + 4)
    }

    /// Calls flush to the underlying writer.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::parser::test::create_test_header;
    use crate::writer::WarcWriter;
    use std::io::Cursor;

    pub fn build_test_warc() -> Vec<u8> {
        const A1: &[u8; 36] = b"Hallo Welt,\n\n das hier ist ein test!";
        const A2: &[u8; 64] =
            b"Ich bin auch eine testfile \n\r\n\rWARC/1.1\r\n Aber das macht nichts!";
        let header = create_test_header("amazon", A1.len() as u64);
        let mut writer = WarcWriter::new(Vec::new());
        writer.write_header(&header).unwrap();
        writer.write_complete_body(A1.as_slice()).unwrap();
        let header = create_test_header("ebay", A2.len() as u64);
        writer.write_header(&header).unwrap();
        writer.write_body(&mut Cursor::new(A2.as_slice())).unwrap();
        writer.into_inner()
    }

    #[test]
    fn can_write() {
        let inner = build_test_warc();
        let data = unsafe { String::from_utf8_unchecked(inner) };
        println!("{}", data.escape_debug())
    }
}
