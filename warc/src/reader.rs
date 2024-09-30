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
use std::fmt::{Debug, Formatter};
use std::io;
use std::io::Read;

use nom::error::ErrorKind;
use nom::Needed;
use strum::EnumString;
use thiserror::Error;
use ubyte::ByteUnit;

use crate::field::{WarcFieldName, WarcFieldValue};
use crate::header::{RequiredFieldError, WarcHeader};
use crate::parser::{parse_warc_header, peek_warc_version, WarcVersionPeek};
use crate::reader::ReadTarget::Header;
use crate::states::State;

pub struct WarcCursor<T> {
    inner: T,
    backlog: Vec<u8>,
    current_header: Option<WarcHeader>,
    current_bytes_in_body: u64,
    state: State,
}

impl<T: Debug> Debug for WarcCursor<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarcCursor")
            .field("inner", &self.inner)
            .field("backlog", &self.backlog)
            .field("current_header", &self.current_header)
            .field("current_bytes_in_body", &self.current_bytes_in_body)
            .field("state", &self.state)
            .finish()
    }
}

impl<T> WarcCursor<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            backlog: Vec::new(),
            current_header: None,
            current_bytes_in_body: 0,
            state: State::ExpectHeader,
        }
    }

    // pub fn header(&self) -> Option<&WarcHeader> {
    //     self.current_header.as_ref()
    // }
    //
    // pub fn state(&self) -> State {
    //     self.state
    // }
    //
    // pub fn into_inner(self) -> T {
    //     self.inner
    // }

    pub fn in_read_body_state(&self) -> bool {
        match self.state {
            State::ExpectHeader => false,
            State::ExpectBody => true,
            State::InBody => true, // State::Corrupt(_) => {false}
        }
    }

    pub fn backlog_shrink(&mut self) {
        self.backlog.shrink_to_fit()
    }

    pub fn backlog_len(&self) -> usize {
        self.backlog.len()
    }

    pub fn backlog_capacity(&self) -> usize {
        self.backlog.capacity()
    }

    // pub fn in_read_header_state(&self) -> bool {
    //     match self.state {
    //         State::ExpectHeader => {true}
    //         State::ExpectBody => {false}
    //         State::InBody => {false}
    //         State::Corrupt(_) => {false}
    //     }
    // }
}

#[derive(Debug, Error)]
pub enum WarcCursorReadError {
    #[error("Tried to read a warc header but the start is not correct, expected: WARC/n.m\\r\\n!")]
    NotAHeader(Vec<u8>),
    #[error("Nom had a recoverable error, but did not manage to recover at {0:?}!")]
    NomError(ErrorKind, Vec<u8>),
    #[error("Nom had a failure at {0:?}!")]
    NomFailure(ErrorKind, Vec<u8>),
    #[error("Encountered an unexpected end of stream after {1:?} for {0:?}!")]
    UnexpectedEos(ReadTarget, usize),
    #[error("The field with the name {0:?} is missing, but it is a required field!")]
    RequiredFieldMissing(WarcFieldName),
    #[error("The field with the name {0:?} is missing, but it is a required field!")]
    RequiredFieldHasIllegalValue(WarcFieldName, WarcFieldValue),
    #[error("The expect a call for {0:?} for {1:?}!")]
    IllegalReadCall(ReadTarget, State),
    #[error("The encountered a bad state {1:?} for {0:?}: {2}")]
    BadStateError(ReadTarget, State, &'static str),
    #[error("Expected the body to end with \\r\\n\\r\\n but it ended with {1:?}!")]
    BadRecordEnd((usize, bool), Vec<u8>),
    #[error(transparent)]
    IOError(#[from] io::Error),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, EnumString)]
pub enum ReadTarget {
    Header,
    Body,
}

impl<T> WarcCursor<T>
where
    T: Read,
{
    // pub fn goto_next_entry(&mut self) -> Result<Option<&WarcHeader>, WarcCursorReadError> {
    //     if self.in_read_header_state() {
    //         return unsafe{self.read_header()};
    //     }
    //     let mut buffer = [0u8; 1024*8];
    //     while self.in_read_body_state() {
    //         let _ = unsafe{self.read_body(&mut buffer)?};
    //     }
    //     if self.in_read_header_state() {
    //         return unsafe{self.read_header()};
    //     } else {
    //         Err(WarcCursorReadError::BadStateError(Header, self.state, "Expected to get to the value"))
    //     }
    // }

    /// Returns true if end of stream is reached (backlog is empty)
    pub fn eos(&mut self) -> io::Result<bool> {
        if !self.backlog.is_empty() {
            return Ok(false);
        }
        let mut buf = [0u8; 32];
        let loaded = self.inner.read(buf.as_mut_slice())?;
        if loaded == 0 {
            Ok(true)
        } else {
            unsafe {
                self.add_to_backlog(&buf[..loaded]);
            }
            Ok(false)
        }
    }

    unsafe fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.backlog.is_empty() {
            self.inner.read(buf)
        } else {
            let drained = self.backlog.drain(..min(buf.len(), self.backlog.len()));
            let sub_buf = drained.len();
            (&mut buf[..drained.len()]).copy_from_slice(drained.as_slice());
            let read = self.inner.read(&mut buf[drained.len()..])?;
            return Ok(sub_buf + read);
        }
    }

    unsafe fn add_to_backlog(&mut self, additional: &[u8]) {
        if additional.is_empty() {
            return;
        }
        self.backlog.extend_from_slice(additional);
    }

    pub unsafe fn read_header(&mut self) -> Result<Option<&WarcHeader>, WarcCursorReadError> {
        if self.state != State::ExpectHeader {
            return Err(WarcCursorReadError::IllegalReadCall(
                ReadTarget::Body,
                self.state,
            ));
        }
        let mut current_data = Vec::with_capacity(1024 * 8);
        let mut buffer = [0u8; 1024 * 8];
        let mut needed: Option<Needed> = None;
        let mut peek_success = false;
        loop {
            let bytes_read = self.read(&mut buffer)?;
            if bytes_read == 0 && needed.is_some() {
                let after = current_data.len();
                self.add_to_backlog(&current_data);
                return if self.backlog.is_empty() {
                    Ok(None)
                } else {
                    Err(WarcCursorReadError::UnexpectedEos(Header, after))
                };
            }
            current_data.extend_from_slice(&buffer[..bytes_read]);
            if !peek_success {
                match peek_warc_version(&current_data) {
                    WarcVersionPeek::NotEnoughBytes => continue,
                    WarcVersionPeek::NotFound => {
                        self.add_to_backlog(&current_data);
                        return Err(WarcCursorReadError::NotAHeader(current_data));
                    }
                    WarcVersionPeek::StartsCorrectly(needs_more) => {
                        if !needs_more {
                            self.add_to_backlog(&current_data);
                            return Err(WarcCursorReadError::NotAHeader(current_data));
                        }
                    }
                    WarcVersionPeek::FirstDigit(needs_more) => {
                        if !needs_more {
                            self.add_to_backlog(&current_data);
                            return Err(WarcCursorReadError::NotAHeader(current_data));
                        }
                    }
                    WarcVersionPeek::Dot(needs_more) => {
                        if !needs_more {
                            self.add_to_backlog(&current_data);
                            return Err(WarcCursorReadError::NotAHeader(current_data));
                        }
                    }
                    WarcVersionPeek::Complete => peek_success = true,
                }
            }
            match parse_warc_header(&current_data) {
                Ok((left, read)) => {
                    self.add_to_backlog(left);
                    self.current_header = Some(read);
                    self.state = State::ExpectBody;
                    return Ok(self.current_header.as_ref());
                }
                Err(err) => match err {
                    nom::Err::Incomplete(n) => {
                        needed = Some(n);
                    }
                    nom::Err::Error(err) => {
                        self.add_to_backlog(&current_data);
                        return Err(WarcCursorReadError::NomError(err.code, current_data));
                    }
                    nom::Err::Failure(fail) => {
                        self.add_to_backlog(&current_data);
                        return Err(WarcCursorReadError::NomFailure(fail.code, current_data));
                    }
                },
            }
        }
    }

    pub unsafe fn get_current_header(
        &mut self,
    ) -> Result<Option<&WarcHeader>, WarcCursorReadError> {
        Ok(if self.state == State::ExpectHeader {
            self.read_header()?
        } else {
            self.current_header.as_ref()
        })
    }

    unsafe fn set_current_bytes_in_body(&mut self) -> Result<(), WarcCursorReadError> {
        if self.current_bytes_in_body == 0 {
            let header = self.get_current_header()?;

            if let Some(header) = header {
                self.current_bytes_in_body = match header.get_content_length() {
                    Ok(found) => *found,
                    Err(RequiredFieldError::NotFound(field_name)) => {
                        return Err(WarcCursorReadError::RequiredFieldMissing(field_name))
                    }
                    Err(RequiredFieldError::WrongType(field_name, problem)) => {
                        return Err(WarcCursorReadError::RequiredFieldHasIllegalValue(
                            field_name,
                            problem.clone(),
                        ))
                    }
                };
            }
        }

        Ok(())
    }

    /// Returns the number of bytes read.
    pub unsafe fn read_body(&mut self, buf: &mut [u8]) -> Result<usize, WarcCursorReadError> {
        if self.get_current_header()?.is_none() {
            return Ok(0);
        }

        self.set_current_bytes_in_body()?;

        match self.state {
            State::ExpectHeader => {
                return Err(WarcCursorReadError::BadStateError(
                    ReadTarget::Body,
                    self.state,
                    "Currently expecting a header!",
                ))
            }
            // State::Corrupt(message) => {
            //     return Err(WarcCursorReadError::BadStateError(ReadTarget::Body, self.state, message))
            // }
            _ => {}
        }

        assert_ne!(0, self.current_bytes_in_body);
        let target_read = min(self.current_bytes_in_body, buf.len() as u64) as usize;
        let read = self.read(&mut buf[..target_read])?;
        assert!(self.current_bytes_in_body >= read as u64);
        self.current_bytes_in_body -= read as u64;
        if self.current_bytes_in_body == 0 {
            self.state = State::ExpectHeader;
            let mut end_buffer = [0u8; 4];
            let end_good = self.read(end_buffer.as_mut_slice())?;
            if end_good == 4 {
                if !end_buffer.eq(b"\r\n\r\n") {
                    Err(WarcCursorReadError::BadRecordEnd(
                        (read, false),
                        end_buffer.to_vec(),
                    ))
                } else {
                    Ok(read)
                }
            } else {
                Err(WarcCursorReadError::BadRecordEnd(
                    (read, false),
                    end_buffer.to_vec(),
                ))
            }
        } else {
            self.state = State::InBody;
            Ok(read)
        }
    }

    pub unsafe fn read_body_complete(&mut self) -> Result<Vec<u8>, WarcCursorReadError> {
        let mut buffer = [0u8; 1024 * 8];
        let mut read_data = Vec::new();
        loop {
            match self.read_body(&mut buffer) {
                Ok(read) => {
                    read_data.extend_from_slice(&buffer[..read]);
                    match self.state {
                        State::ExpectHeader => break,
                        // State::Corrupt(message) => {
                        //     return Err(WarcCursorReadError::BadStateError(ReadTarget::Body, self.state, message))
                        // }
                        _ => {}
                    }
                }
                Err(WarcCursorReadError::BadRecordEnd((read, _), _)) => {
                    read_data.extend_from_slice(&buffer[..read]);
                    break;
                }
                Err(other) => return Err(other),
            }
        }
        Ok(read_data)
    }

    pub fn read_entry(&mut self) -> Result<Option<(WarcHeader, Body<T>)>, WarcCursorReadError> {
        if self.state != State::ExpectHeader {
            return Err(WarcCursorReadError::BadStateError(
                Header,
                self.state,
                "When reading an entry, the state has to be expecting a header first!",
            ));
        }
        unsafe {
            match self.get_current_header()?.cloned() {
                None => Ok(None),
                Some(header) => {
                    self.set_current_bytes_in_body()?;
                    if self.current_bytes_in_body > ByteUnit::Megabyte(10).as_u64() {
                        return Ok(Some((header, Body::Complete(self.read_body_complete()?))));
                    } else {
                        return Ok(Some((header, Body::Partial(BodyReader::new(self)))));
                    }
                }
            }
        }
    }

    pub fn read_or_get_header(&mut self) -> Result<Option<&WarcHeader>, WarcCursorReadError> {
        if self.state != State::ExpectHeader {
            return Err(WarcCursorReadError::BadStateError(
                Header,
                self.state,
                "When reading an entry, the state has to be expecting a header first!",
            ));
        }

        unsafe { self.get_current_header() }
    }
}

// impl<T> WarcCursor<T> where T: Seek {
//     unsafe fn clear_all_states(&mut self) {
//         self.current_bytes_in_body = 0;
//         self.state = State::ExpectHeader;
//         self.backlog.clear();
//         self.current_header = None;
//     }
//
//     pub unsafe fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
//         self.clear_all_states();
//         self.inner.seek(pos)
//     }
//
//     pub fn rewind(&mut self) -> io::Result<()>{
//         unsafe{self.clear_all_states();}
//         self.inner.rewind()
//     }
// }

pub enum Body<'a, T: Read> {
    Complete(Vec<u8>),
    Partial(BodyReader<'a, T>),
}

impl<'a, T: Read> Body<'a, T> {
    pub fn load_completely(self) -> Result<Vec<u8>, WarcCursorReadError> {
        match self {
            Body::Complete(value) => Ok(value),
            Body::Partial(mut value) => {
                let mut data = Vec::new();
                while let Some(dat) = value.read_next()? {
                    data.extend_from_slice(dat)
                }
                Ok(data)
            }
        }
    }
}

pub struct BodyReader<'a, T: Read> {
    parent: &'a mut WarcCursor<T>,
    bytes_read: usize,
    buffer: [u8; 1024 * 8],
}

impl<'a, T: Read + Debug> Debug for BodyReader<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BodyReader")
            .field("parent", &self.parent)
            .field("bytes_read", &self.bytes_read)
            .field("buffer", &self.buffer)
            .finish()
    }
}

impl<'a, T> BodyReader<'a, T>
where
    T: Read,
{
    fn new(parent: &'a mut WarcCursor<T>) -> Self {
        Self {
            parent,
            buffer: [0u8; 1024 * 8],
            bytes_read: 0,
        }
    }

    pub fn read_next(&mut self) -> Result<Option<&[u8]>, WarcCursorReadError> {
        if !self.parent.in_read_body_state() {
            return Ok(None);
        }
        unsafe {
            let bytes_read = self.parent.read_body(&mut self.buffer)?;
            self.bytes_read = bytes_read;
            return Ok(Some(&self.buffer[..bytes_read]));
        }
    }

    // pub fn get_current(&self) -> Option<&[u8]> {
    //     if self.bytes_read == 0 {
    //         None
    //     } else {
    //         Some(&self.buffer[..self.bytes_read])
    //     }
    // }

    // pub fn clear_current(&mut self) {
    //     self.bytes_read = 0;
    //     self.buffer.as_mut_slice().fill(0);
    // }
}

#[cfg(test)]
mod test {
    use crate::reader::{WarcCursor, WarcCursorReadError};
    use crate::writer::test::build_test_warc;
    use std::io::Cursor;

    #[test]
    fn can_read() {
        let to_read = build_test_warc();
        let mut cursor = WarcCursor::new(Cursor::new(to_read));
        println!("C1: {cursor:?}");

        match cursor.read_entry() {
            Ok(result) => {
                let (header, body) = result.expect("Why none?");
                println!("Header:\n---\n---{header}---\n---");
                println!("Body:\n---\n---{}---\n---", unsafe {
                    String::from_utf8_unchecked(body.load_completely().unwrap())
                });
            }
            Err(err) => match err {
                WarcCursorReadError::NotAHeader(value) => {
                    panic!("NAH {}", unsafe { String::from_utf8_unchecked(value) })
                }
                others => panic!("{}", others),
            },
        }

        println!("\n\n");

        match cursor.read_entry() {
            Ok(result) => {
                let (header, body) = result.expect("Why none?");
                println!("Header:\n---\n---{header}---\n---");
                println!("Body:\n---\n---{}---\n---", unsafe {
                    String::from_utf8_unchecked(body.load_completely().unwrap())
                });
            }
            Err(err) => match err {
                WarcCursorReadError::NotAHeader(value) => {
                    panic!("NAH {}", unsafe { String::from_utf8_unchecked(value) })
                }
                others => panic!("{}", others),
            },
        }

        println!("{}", cursor.eos().unwrap());
    }
}
