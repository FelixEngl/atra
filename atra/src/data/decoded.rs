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

use encoding_rs::Encoding;
use std::path::Path;

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
pub enum Decoded<A, B>
where
    A: AsRef<str>,
    B: AsRef<Path>,
{
    InMemory {
        data: A,
        encoding: &'static Encoding,
        had_errors: bool,
    },
    OffMemory {
        reference: B,
        encoding: &'static Encoding,
        had_errors: bool,
    },
    None,
}

impl<A, B> Decoded<A, B>
where
    A: AsRef<str>,
    B: AsRef<Path>,
{
    #[inline]
    pub fn new_in_memory(result: A, encoding: &'static Encoding, had_errors: bool) -> Self {
        Self::InMemory {
            data: result,
            encoding,
            had_errors,
        }
    }

    #[inline]
    pub fn new_off_memory(result: B, encoding: &'static Encoding, had_errors: bool) -> Self {
        Self::OffMemory {
            reference: result,
            encoding,
            had_errors,
        }
    }

    #[cfg(test)]
    pub fn as_in_memory(&self) -> Option<&A> {
        match self {
            Decoded::InMemory { data: result, .. } => Some(result),
            Decoded::OffMemory { .. } => None,
            Decoded::None => None,
        }
    }

    pub fn encoding(&self) -> Option<&'static Encoding> {
        match self {
            Decoded::InMemory { encoding, .. } => Some(*encoding),
            Decoded::OffMemory { encoding, .. } => Some(*encoding),
            Decoded::None => None,
        }
    }

    pub fn had_errors(&self) -> bool {
        match self {
            Decoded::InMemory { had_errors, .. } => *had_errors,
            Decoded::OffMemory { had_errors, .. } => *had_errors,
            Decoded::None => false,
        }
    }

    pub fn map_in_memory<R: AsRef<str>, F>(self, block: F) -> Decoded<R, B>
    where
        F: FnOnce(A) -> R,
    {
        match self {
            Decoded::InMemory {
                data: result,
                encoding,
                had_errors,
            } => Decoded::InMemory {
                data: block(result),
                encoding,
                had_errors,
            },
            Decoded::OffMemory {
                reference: result,
                encoding,
                had_errors,
            } => Decoded::OffMemory {
                reference: result,
                encoding,
                had_errors,
            },
            Decoded::None => Decoded::None,
        }
    }
}

impl<A, B> From<(A, &'static Encoding, bool)> for Decoded<A, B>
where
    A: AsRef<str>,
    B: AsRef<Path>,
{
    fn from(value: (A, &'static Encoding, bool)) -> Self {
        Self::InMemory {
            data: value.0,
            encoding: value.1,
            had_errors: value.2,
        }
    }
}

impl<A, B> Clone for Decoded<A, B>
where
    A: AsRef<str> + Clone,
    B: AsRef<Path> + Clone,
{
    fn clone(&self) -> Self {
        match self {
            Decoded::InMemory {
                data: result,
                encoding,
                had_errors,
            } => Decoded::InMemory {
                data: result.clone(),
                encoding: *encoding,
                had_errors: *had_errors,
            },
            Decoded::OffMemory {
                reference: result,
                encoding,
                had_errors,
            } => Decoded::OffMemory {
                reference: result.clone(),
                encoding: *encoding,
                had_errors: *had_errors,
            },
            Decoded::None => Decoded::None,
        }
    }
}
