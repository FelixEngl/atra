use std::cmp::min;
use std::io::{Error, Read, Seek, SeekFrom};

use ubyte::ByteUnit;

use crate::warc_ext::skip_pointer::WarcSkipPointer;

/// Reads the body from [reader] for a provided [pointer]
pub fn read_body<R: Seek + Read>(
    reader: &mut R,
    pointer: &WarcSkipPointer,
    header_octet_count: u32,
) -> Result<Option<Vec<u8>>, Error> {
    let header_octet_count = header_octet_count as u64;
    reader.seek(SeekFrom::Start(
        pointer.position() + pointer.warc_header_offset() as u64 + header_octet_count,
    ))?;
    let mut to_read = (pointer.body_octet_count() - header_octet_count) as usize;
    if to_read == 0 {
        return Ok(None);
    }

    let mut data = Vec::new();
    const BUF_SIZE: usize = ByteUnit::Megabyte(2).as_u64() as usize;
    let buffer = &mut [0u8; BUF_SIZE];
    while data.len() < to_read {
        reader.read(&mut buffer[..min(BUF_SIZE, to_read)])?;
        data.extend_from_slice(&buffer[..min(BUF_SIZE, to_read)]);
        to_read = to_read.saturating_sub(BUF_SIZE);
    }
    return Ok(Some(data));
}
