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

use std::io::{Error, Read, Seek, SeekFrom};
use warc::header::WarcHeader;
use warc::reader::{WarcCursor, WarcCursorReadError};
use crate::warc_ext::skip_pointer::WarcSkipPointer;

/// Reads the body from [reader] for a provided [pointer]
pub fn read_body<R: Seek + Read>(
    reader: &mut R,
    pointer: &WarcSkipPointer,
    header_octet_count: u32,
) -> Result<Option<Vec<u8>>, Error> {
    let header_octet_count = header_octet_count as u64;
    reader.seek(SeekFrom::Start(
        pointer.file_offset() + pointer.warc_header_octet_count() as u64 + header_octet_count,
    ))?;
    let to_read = pointer.body_octet_count() - header_octet_count;
    if to_read == 0 {
        return Ok(None);
    }
    let mut data = Vec::new();
    reader.take(to_read).read_to_end(&mut data)?;
    return Ok(Some(data));
}

/// Reads the meta from [reader] for the [pointer].
pub fn read_meta<R: Seek + Read>(
    reader: &mut R,
    pointer: &WarcSkipPointer,
) -> Result<Option<WarcHeader>, WarcCursorReadError> {
    reader.seek(SeekFrom::Start(
        pointer.file_offset()
    ))?;

    WarcCursor::new(reader)
        .read_or_get_header()
        .map(|value| value.cloned())
}