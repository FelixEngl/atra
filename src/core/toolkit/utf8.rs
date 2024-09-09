use std::io::{BufRead, ErrorKind, Read};
use std::marker::PhantomData;
use byteorder::ReadBytesExt;

#[cfg(RUSTC_IS_NIGHTLY)]
#[feature(str_internals)]
pub use core::str::utf8_char_width;
#[cfg(RUSTC_IS_NIGHTLY)]
#[feature(str_internals)]
pub use core::str::next_code_point;
use std::collections::VecDeque;
use std::str::Utf8Error;
use thiserror::Error;

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

    /// Checks whether the byte is a UTF-8 continuation byte (i.e., starts with the
    /// bits `10`).
    #[inline]
    const fn utf8_is_cont_byte(byte: u8) -> bool {
        (byte as i8) < -64
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

#[cfg(not(RUSTC_IS_NIGHTLY))]
pub use char_ct::{utf8_char_width, next_code_point};
use crate::core::toolkit::utf8::Utf8ReaderError::InvalidSequence;

#[derive(Debug, Error)]
pub enum Utf8ReaderError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    #[error("An invalid marker was found!")]
    InvalidSequence,
}




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

impl<'a, I> From<RobustUtf8Reader<'a, I>> for Utf8Reader<'a, I>  {
    fn from(value: RobustUtf8Reader<'a, I>) -> Self {
        Self { inner: value, invalid_seq_error: false, stopped: false }
    }

}


impl<'a, I> Iterator for Utf8Reader<'a, I> where I: Read  {
    type Item = Result<char, Utf8ReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None
        }
        if self.invalid_seq_error {
            self.stopped = true;
            return Some(Err(InvalidSequence))
        }
        match self.inner.next() {
            None => {
                self.stopped = true;
                None
            }
            Some(next) => {
                match next {
                    Ok(DecodedChar{ ch: value, encountered_only_valid: false}) => {
                        self.invalid_seq_error = true;
                        Some(Ok(value))
                    },
                    Ok(DecodedChar{ ch: value, encountered_only_valid: true}) => {
                        Some(Ok(value))
                    }
                    Err(value) => {
                        Some(Err(value.into()))
                    }
                }
            }
        }
    }
}


pub struct RobustUtf8Reader<'a, R> {
    input: R,
    stopped: bool,
    memory: VecDeque<u8>,
    _lifeline: PhantomData<&'a ()>
}

impl<'a, R> RobustUtf8Reader<'a, R> {
    const MEMORY_CAPACITY: usize = 7;
    const MIN_MEMORY_SIZE: usize = 4;

    pub fn new(input: R) -> Self {
        Self { input, stopped: false, memory: VecDeque::with_capacity(Self::MEMORY_CAPACITY), _lifeline: PhantomData }
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }
}

impl<'a, R> RobustUtf8Reader<'a, R> where R: Read  {
    fn fill_memory(&mut self) -> Result<(), std::io::Error>{
        if Self::MIN_MEMORY_SIZE <= self.memory.len() {
            Ok(())
        } else {
            let mut buf = [0u8; Self::MEMORY_CAPACITY];
            match self.input.read(&mut buf[0..Self::MEMORY_CAPACITY - self.memory.len()]) {
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
                Err(other) => {
                    return Err(other)
                }
            }

            Ok(())
        }
    }

    /// Returns false if not all value
    fn fill_buffer(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        debug_assert!(buf.len() < 5);
        for i in 0..buf.len() {
            if let Some(found) = self.memory.pop_front() {
                buf[i] = found
            } else {
                self.fill_memory()?;
                if let Some(found) = self.memory.pop_front() {
                    buf[i] = found
                } else {
                    return Ok(i)
                }
            }
        }
        Ok(buf.len())
    }

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

    fn pop_front_safe(&mut self) -> Result<Option<u8>, std::io::Error> {
        if let Some(value) = self.memory.pop_front() {
            Ok(Some(value))
        } else {
            self.fill_memory()?;
            Ok(self.memory.pop_front())
        }
    }
}

/// The boolean is false if the sequence before this character contained illegal codepoints.
#[derive(Debug, Copy, Clone)]
pub struct DecodedChar{
    pub ch: char,
    pub encountered_only_valid: bool
}

impl DecodedChar {
    pub const fn new(c: char, encountered_only_valid: bool) -> Self {
        Self { ch: c, encountered_only_valid }
    }
}

impl<'a, R> Iterator for RobustUtf8Reader<'a, R> where R: Read  {
    type Item = Result<DecodedChar, std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None
        }
        match self.fill_memory() {
            Ok(_) => {
                if self.memory.is_empty() {
                    self.stopped = true;
                    return None
                }
                let mut encountered_only_valid = true;
                while let Some(byte0) = self.pop_front_safe().transpose() {
                    match byte0 {
                        Ok(byte0) => {
                            if byte0 < 128 {
                                let c = unsafe { char::from_u32_unchecked(byte0 as u32) };
                                return Some(Ok(DecodedChar::new(c, encountered_only_valid)))
                            }
                            let mut buf = [byte0, 0u8, 0u8, 0u8];
                            let expected_char_width = utf8_char_width(byte0);
                            debug_assert_ne!(expected_char_width, 1);
                            match expected_char_width {
                                2 => {
                                    match self.fill_buffer(&mut buf[1..=1]) {
                                        Ok(0) => {
                                            debug_assert!(self.memory.is_empty());
                                            self.stopped = true;
                                            return None
                                        }
                                        Ok(1) => {
                                            debug_assert_eq!(expected_char_width, 2)
                                        }
                                        Err(err) => {
                                            return Some(Err(err))
                                        }
                                        Ok(_) => unreachable!()
                                    }
                                }
                                3 => {
                                    match self.fill_buffer(&mut buf[1..3]) {
                                        Ok(0) => {
                                            debug_assert!(self.memory.is_empty());
                                            self.stopped = true;
                                            return None
                                        }
                                        Ok(1) => {
                                            self.push_back_to_front(&buf[1..=1]);
                                            encountered_only_valid = false;
                                            continue
                                        },
                                        Ok(2) => {
                                            debug_assert_eq!(expected_char_width, 3)
                                        }
                                        Err(err) => {
                                            return Some(Err(err))
                                        }
                                        Ok(_) => unreachable!()
                                    }
                                }
                                4 => {
                                    match self.fill_buffer(&mut buf[1..4]) {
                                        Ok(0) => {
                                            debug_assert!(self.memory.is_empty());
                                            self.stopped = true;
                                            return None
                                        }
                                        Ok(1) => {
                                            self.push_back_to_front(&buf[1..=1]);
                                            encountered_only_valid = false;
                                            continue
                                        }
                                        Ok(2) => {
                                            self.push_back_to_front(&buf[1..=2]);
                                            encountered_only_valid = false;
                                            continue
                                        }
                                        Ok(3) => {
                                            debug_assert_eq!(expected_char_width, 4)
                                        }
                                        Err(err) => {
                                            return Some(Err(err))
                                        }
                                        Ok(_) => unreachable!()
                                    }
                                }
                                _ => {
                                    encountered_only_valid = false;
                                    continue
                                },
                            }
                            unsafe {
                                match next_code_point(&mut buf.iter()).map(char::from_u32).flatten() {
                                    None => {
                                        self.push_back_to_front(&buf[1..expected_char_width]);
                                        encountered_only_valid = false;
                                        continue
                                    }
                                    Some(value) => {
                                        return Some(Ok(DecodedChar::new(value, encountered_only_valid)))
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            self.stopped = true;
                            return Some(Err(err))
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
    use std::io::Cursor;
    use itertools::Itertools;
    use crate::core::toolkit::utf8::{DecodedChar, RobustUtf8Reader, Utf8Reader};

    #[test]
    fn test_normal(){
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
    fn test_robust(){
        let s = "abc СФҬ ᡄ᱖⓹ 𒀀𒉱📂".to_string();
        let mut v = s.clone().into_bytes();
        v.insert(1, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.insert(1, 0b1000_0000);
        v.push(0b1000_0000);
        let cursor = RobustUtf8Reader::new(Cursor::new(v));
        for (value, c_original) in cursor.zip_eq(s.chars()) {
            let DecodedChar{ ch: value, encountered_only_valid: err } = value.unwrap();
            println!("{c_original}: {value} - {err}");
            assert_eq!(c_original, value);
        }
    }
}