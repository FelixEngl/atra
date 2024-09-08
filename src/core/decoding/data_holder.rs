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

use std::path::Path;
use encoding_rs::Encoding;

/// This method implements the (non-streaming version of) the
/// [_decode_](https://encoding.spec.whatwg.org/#decode) spec concept.
///
/// The second item in the returned tuple is the encoding that was actually
/// used (which may differ from this encoding thanks to BOM sniffing).
///
/// The third item in the returned tuple indicates whether there were
/// malformed sequences (that were replaced with the REPLACEMENT CHARACTER).
///
/// _Note:_ It is wrong to use this when the input buffer represents only
/// a segment of the input instead of the whole input. Use `new_decoder()`
/// when decoding segmented input.

#[derive(Debug)]
pub enum DecodedData<A, B> where A: AsRef<str>, B: AsRef<Path> {
    InMemory {
        result: A,
        encoding: &'static Encoding,
        had_errors: bool
    },
    OffMemory {
        result: B,
        encoding: &'static Encoding,
        had_errors: bool
    },
    None
}

impl<A, B> DecodedData<A, B> where A: AsRef<str>, B: AsRef<Path> {

    #[inline]
    #[allow(dead_code)] pub fn new_in_memory(result: A, encoding: &'static Encoding, had_errors: bool) -> Self {
        Self::InMemory {
            result,
            encoding,
            had_errors
        }
    }

    #[inline]
    #[allow(dead_code)] pub fn new_off_memory(result: B, encoding: &'static Encoding, had_errors: bool) -> Self {
        Self::OffMemory {
            result,
            encoding,
            had_errors
        }
    }

    #[cfg(test)]
    pub fn as_in_memory(&self) -> Option<&A> {
        match self {
            DecodedData::InMemory { result, .. } => {Some(result)}
            DecodedData::OffMemory { .. } => {None}
            DecodedData::None => {None}
        }
    }

    pub fn encoding(&self) -> Option<&'static Encoding> {
        match self {
            DecodedData::InMemory { encoding, .. } => {Some(*encoding)}
            DecodedData::OffMemory { encoding, .. } => {Some(*encoding)}
            DecodedData::None => {None}
        }
    }

    pub fn had_errors(&self) -> bool {
        match self {
            DecodedData::InMemory { had_errors, .. } => {*had_errors}
            DecodedData::OffMemory { had_errors, .. } => {*had_errors}
            DecodedData::None => {false}
        }
    }


    pub fn map_in_memory<R: AsRef<str>, F>(self, block: F) -> DecodedData<R, B> where F: FnOnce(A) -> R {
        match self {
            DecodedData::InMemory { result, encoding, had_errors } => {
                DecodedData::InMemory {
                    result: block(result),
                    encoding,
                    had_errors
                }
            }
            DecodedData::OffMemory {result, encoding, had_errors} => {
                DecodedData::OffMemory {result, encoding, had_errors}
            }
            DecodedData::None => DecodedData::None
        }
    }
}

impl<A, B> From<(A, &'static Encoding, bool)> for DecodedData<A, B> where A: AsRef<str>, B: AsRef<Path>  {
    fn from(value: (A, &'static Encoding, bool)) -> Self {
        Self::InMemory {
            result: value.0,
            encoding: value.1,
            had_errors: value.2
        }
    }
}


impl<A, B> Clone for DecodedData<A, B> where A: AsRef<str> + Clone, B: AsRef<Path> + Clone {
    fn clone(&self) -> Self {
        match self {
            DecodedData::InMemory { result, encoding, had_errors } => {
                DecodedData::InMemory {
                    result: result.clone(),
                    encoding: *encoding,
                    had_errors: *had_errors
                }
            }
            DecodedData::OffMemory { result, encoding, had_errors } => {
                DecodedData::OffMemory {
                    result: result.clone(),
                    encoding: *encoding,
                    had_errors: *had_errors
                }
            }
            DecodedData::None => { DecodedData::None}
        }
    }
}
