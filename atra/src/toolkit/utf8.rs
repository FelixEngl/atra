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

use std::io::{ErrorKind, Read};
use std::marker::PhantomData;

#[cfg(RUSTC_IS_NIGHTLY)]
#[feature(str_internals)]
pub use core::str::next_code_point;
#[cfg(RUSTC_IS_NIGHTLY)]
#[feature(str_internals)]
pub use core::str::utf8_char_width;
use std::collections::VecDeque;
use std::str::Utf8Error;
use thiserror::Error;

/// A decoded character with some meta information about its context.
#[derive(Debug, Copy, Clone)]
pub struct DecodedChar {
    /// The decoded utf8 character
    pub ch: char,
    /// Is zero when no errors occur between the characters.
    pub invalid_encounters: usize,
}

impl DecodedChar {
    /// Creates a new decoded char.
    #[inline(always)]
    pub const fn new(c: char, invalid_encounters: usize) -> Self {
        Self {
            ch: c,
            invalid_encounters,
        }
    }

    /// Returns true if only valid values are encountered.
    #[inline(always)]
    pub const fn encountered_only_valid(&self) -> bool {
        self.invalid_encounters == 0
    }
}

#[cfg(not(RUSTC_IS_NIGHTLY))]
mod char_ct {
    // https://tools.ietf.org/html/rfc3629
    const UTF8_CHAR_WIDTH: &[u8; 256] = &[
        // 1  2  3  4  5  6  7  8  9  A  B  C  D  E  F
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 1
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 2
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 3
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 4
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 5
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 6
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 7
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 8
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 9
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // A
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // B
        0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // C
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // D
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // E
        4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // F
    ];

    const CONT_MASK: u8 = 0b0011_1111;

    /// Given a first byte, determines how many bytes are in this UTF-8 character.
    #[inline]
    pub const fn utf8_char_width(b: u8) -> usize {
        UTF8_CHAR_WIDTH[b as usize] as usize
    }

    /// Returns the initial codepoint accumulator for the first byte.
    /// The first byte is special, only want bottom 5 bits for width 2, 4 bits
    /// for width 3, and 3 bits for width 4.
    #[inline]
    const fn utf8_first_byte(byte: u8, width: u32) -> u32 {
        (byte & (0x7F >> width)) as u32
    }

    /// Returns the value of `ch` updated with continuation byte `byte`.
    #[inline]
    const fn utf8_acc_cont_byte(ch: u32, byte: u8) -> u32 {
        (ch << 6) | (byte & CONT_MASK) as u32
    }

    /// Reads the next code point out of a byte iterator (assuming a
    /// UTF-8-like encoding).
    ///
    /// # Safety
    ///
    /// `bytes` must produce a valid UTF-8-like (UTF-8 or WTF-8) string
    #[inline]
    pub unsafe fn next_code_point<'a, I: Iterator<Item = &'a u8>>(bytes: &mut I) -> Option<u32> {
        // Decode UTF-8
        let x = *bytes.next()?;
        if x < 128 {
            return Some(x as u32);
        }

        // Multibyte case follows
        // Decode from a byte combination out of: [[[x y] z] w]
        // NOTE: Performance is sensitive to the exact formulation here
        let init = utf8_first_byte(x, 2);
        // SAFETY: `bytes` produces an UTF-8-like string,
        // so the iterator must produce a value here.
        let y = unsafe { *bytes.next().unwrap_unchecked() };
        let mut ch = utf8_acc_cont_byte(init, y);
        if x >= 0xE0 {
            // [[x y z] w] case
            // 5th bit in 0xE0 .. 0xEF is always clear, so `init` is still valid
            // SAFETY: `bytes` produces an UTF-8-like string,
            // so the iterator must produce a value here.
            let z = unsafe { *bytes.next().unwrap_unchecked() };
            let y_z = utf8_acc_cont_byte((y & CONT_MASK) as u32, z);
            ch = init << 12 | y_z;
            if x >= 0xF0 {
                // [x y z w] case
                // use only the lower 3 bits of `init`
                // SAFETY: `bytes` produces an UTF-8-like string,
                // so the iterator must produce a value here.
                let w = unsafe { *bytes.next().unwrap_unchecked() };
                ch = (init & 7) << 18 | utf8_acc_cont_byte(y_z, w);
            }
        }

        Some(ch)
    }
}

use crate::toolkit::utf8::Utf8ReaderError::InvalidSequence;
#[cfg(not(RUSTC_IS_NIGHTLY))]
pub use char_ct::{next_code_point, utf8_char_width};

#[derive(Debug, Error)]
pub enum Utf8ReaderError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error("An invalid marker was found!")]
    InvalidSequence,
}

/// Tries to read something as an utf8, stops when failing
pub struct Utf8Reader<'a, I> {
    inner: RobustUtf8Reader<'a, I>,
    invalid_seq_error: bool,
    stopped: bool,
}

impl<'a, I> Utf8Reader<'a, I> {
    pub fn new(input: I) -> Self {
        RobustUtf8Reader::new(input).into()
    }

    pub fn into_inner(self) -> RobustUtf8Reader<'a, I> {
        self.inner
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }
}

impl<'a, I> From<RobustUtf8Reader<'a, I>> for Utf8Reader<'a, I> {
    fn from(value: RobustUtf8Reader<'a, I>) -> Self {
        Self {
            inner: value,
            invalid_seq_error: false,
            stopped: false,
        }
    }
}

impl<'a, I> Iterator for Utf8Reader<'a, I>
where
    I: Read,
{
    type Item = Result<char, Utf8ReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        if self.invalid_seq_error {
            self.stopped = true;
            return Some(Err(InvalidSequence));
        }
        match self.inner.next() {
            None => {
                self.stopped = true;
                None
            }
            Some(next) => match next {
                Ok(DecodedChar {
                    ch: value,
                    invalid_encounters: 0,
                }) => Some(Ok(value)),
                Ok(DecodedChar { ch: value, .. }) => {
                    self.invalid_seq_error = true;
                    Some(Ok(value))
                }
                Err(value) => Some(Err(value.into())),
            },
        }
    }
}

/// Tries to read a reader as utf8. Iff it is not capable to read something as character it skips
/// to the next
pub struct RobustUtf8Reader<'a, R> {
    input: R,
    stopped: bool,
    memory: VecDeque<u8>,
    _lifeline: PhantomData<&'a ()>,
}

/// The capacity of the buffer used.
const MEMORY_CAPACITY: usize = 7;
/// Minimum amount of bytes needed so make sure we have a valid char.
const MIN_MEMORY_SIZE: usize = 4;

impl<'a, R> RobustUtf8Reader<'a, R> {
    /// Creates a new reader.
    pub fn new(input: R) -> Self {
        Self {
            input,
            stopped: false,
            memory: VecDeque::with_capacity(MEMORY_CAPACITY),
            _lifeline: PhantomData,
        }
    }

    /// Returns true if the readeris stopped.
    pub fn stopped(&self) -> bool {
        self.stopped
    }
}

impl<'a, R> RobustUtf8Reader<'a, R>
where
    R: Read,
{
    /// Fill the buffer with bytes from the reader.
    fn fill_memory(&mut self) -> Result<(), std::io::Error> {
        if MIN_MEMORY_SIZE <= self.memory.len() {
            Ok(())
        } else {
            let mut buf = [0u8; MEMORY_CAPACITY];
            match self
                .input
                .read(&mut buf[0..MEMORY_CAPACITY - self.memory.len()])
            {
                Ok(read) => {
                    if read != 0 {
                        for value in buf.iter().take(read) {
                            self.memory.push_back(*value)
                        }
                    }
                }
                Err(err) if err.kind() == ErrorKind::UnexpectedEof => {
                    // Consume eof
                }
                Err(other) => return Err(other),
            }

            Ok(())
        }
    }

    /// Returns false if not all value
    fn fill_buffer(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        debug_assert!(buf.len() < 5);
        if self.memory.len() < buf.len() {
            self.fill_memory()?;
        }

        for i in 0..buf.len() {
            if let Some(found) = self.memory.pop_front() {
                buf[i] = found
            } else {
                self.fill_memory()?;
                if let Some(found) = self.memory.pop_front() {
                    buf[i] = found
                } else {
                    return Ok(i);
                }
            }
        }
        Ok(buf.len())
    }

    /// Pushes some bytes back to the front.
    fn push_back_to_front(&mut self, buf: &[u8]) {
        match buf.len() {
            0 => {}
            1 => self.memory.push_front(buf[0]),
            _ => {
                for b in buf.iter().rev() {
                    self.memory.push_front(*b)
                }
            }
        }
    }

    /// Try to pop the front.
    fn pop_front_safe(&mut self) -> Result<Option<u8>, std::io::Error> {
        if let Some(value) = self.memory.pop_front() {
            Ok(Some(value))
        } else {
            self.fill_memory()?;
            Ok(self.memory.pop_front())
        }
    }
}

impl<'a, R> Iterator for RobustUtf8Reader<'a, R>
where
    R: Read,
{
    type Item = Result<DecodedChar, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        match self.fill_memory() {
            Ok(_) => {
                if self.memory.is_empty() {
                    self.stopped = true;
                    return None;
                }
                let mut invalid_encounters: usize = 0;
                while let Some(byte0) = self.pop_front_safe().transpose() {
                    match byte0 {
                        Ok(byte0) => {
                            if byte0 < 128 {
                                let c = unsafe { char::from_u32_unchecked(byte0 as u32) };
                                return Some(Ok(DecodedChar::new(c, invalid_encounters)));
                            }
                            let mut buf = [byte0, 0u8, 0u8, 0u8];
                            let expected_char_width = utf8_char_width(byte0);
                            debug_assert_ne!(expected_char_width, 1);
                            match expected_char_width {
                                2 => match self.fill_buffer(&mut buf[1..=1]) {
                                    Ok(0) => {
                                        debug_assert!(self.memory.is_empty());
                                        self.stopped = true;
                                        return None;
                                    }
                                    Ok(1) => {}
                                    Err(err) => return Some(Err(err)),
                                    Ok(_) => unreachable!(),
                                },
                                3 => match self.fill_buffer(&mut buf[1..3]) {
                                    Ok(0) => {
                                        debug_assert!(self.memory.is_empty());
                                        self.stopped = true;
                                        return None;
                                    }
                                    Ok(1) => {
                                        self.push_back_to_front(&buf[1..=1]);
                                        invalid_encounters += 1;
                                        continue;
                                    }
                                    Ok(2) => {}
                                    Err(err) => return Some(Err(err)),
                                    Ok(_) => unreachable!(),
                                },
                                4 => match self.fill_buffer(&mut buf[1..4]) {
                                    Ok(0) => {
                                        debug_assert!(self.memory.is_empty());
                                        self.stopped = true;
                                        return None;
                                    }
                                    Ok(1) => {
                                        self.push_back_to_front(&buf[1..=1]);
                                        invalid_encounters += 1;
                                        continue;
                                    }
                                    Ok(2) => {
                                        self.push_back_to_front(&buf[1..=2]);
                                        invalid_encounters += 1;
                                        continue;
                                    }
                                    Ok(3) => {}
                                    Err(err) => return Some(Err(err)),
                                    Ok(_) => unreachable!(),
                                },
                                _ => {
                                    invalid_encounters += 1;
                                    continue;
                                }
                            }
                            unsafe {
                                match next_code_point(&mut buf.iter())
                                    .map(char::from_u32)
                                    .flatten()
                                {
                                    None => {
                                        self.push_back_to_front(&buf[1..expected_char_width]);
                                        invalid_encounters += 1;
                                        continue;
                                    }
                                    Some(value) => {
                                        return Some(Ok(DecodedChar::new(
                                            value,
                                            invalid_encounters,
                                        )))
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            self.stopped = true;
                            return Some(Err(err));
                        }
                    }
                }
                self.stopped = true;
                None
            }
            Err(err) => {
                self.stopped = true;
                Some(Err(err))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::toolkit::utf8::{DecodedChar, RobustUtf8Reader, Utf8Reader};
    use itertools::Itertools;
    use std::io::Cursor;

    #[test]
    fn test_normal() {
        let s = "abc СФҬ ᡄ᱖⓹ 𒀀𒉱📂".to_string();
        let cursor1 = Utf8Reader::new(Cursor::new(Vec::from(s.clone())));
        for v in cursor1 {
            println!("{v:?}");
        }
        let cursor = Utf8Reader::new(Cursor::new(Vec::from(s.clone())));
        for (value, c_original) in cursor.zip_eq(s.chars()) {
            assert_eq!(Some(c_original), value.ok())
        }
    }

    #[test]
    fn test_robust() {
        let s = "abc СФҬ ᡄ᱖⓹ 𒀀𒉱📂".to_string();
        let mut v = s.clone().into_bytes();
        v.insert(1, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.insert(4, 0b1000_0000);
        v.insert(5, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.push(0b1000_0000);
        let cursor = RobustUtf8Reader::new(Cursor::new(v));
        for (value, c_original) in cursor.zip_eq(s.chars()) {
            let DecodedChar {
                ch: value,
                invalid_encounters: err,
            } = value.unwrap();
            println!("{c_original}: {value} - {err}");
            assert_eq!(c_original, value);
        }
    }
}
