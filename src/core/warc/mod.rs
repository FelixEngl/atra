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

pub mod writer;
pub mod skip_pointer;

use std::borrow::Cow;
use std::cmp::min;
use std::fs::File;
use std::io::{Error, Read, Seek, SeekFrom};
use data_encoding::{BASE64, DecodeError};
use uuid::Uuid;
use itertools::{Itertools, Position};
use reqwest::header::{CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ubyte::{ByteUnit, ToByteUnit};
use crate::core::crawl::result::CrawlResult;
use crate::core::{VecDataHolder};
use crate::core::database_error::DatabaseError;
use crate::core::digest::labeled_xxh128_digest;
use crate::core::format::supported::AtraSupportedFileFormat;
use crate::core::io::errors::{ErrorWithPath, ToErrorWithPath};
use crate::core::io::file_owner::FileOwner;
use crate::core::warc::skip_pointer::{WarcSkipPointer, WarcSkipPointerWithPath};
use crate::warc::media_type::{MediaType, parse_media_type};
use crate::warc::header::{WarcHeader};
use crate::warc::field::{UriLikeFieldValue};
use crate::warc::record_type::WarcRecordType;
use crate::warc::truncated_reason::TruncatedReason;

pub use self::writer::SpecialWarcWriter;
#[cfg(test)]
pub use self::writer::MockSpecialWarcWriter;

/// An instruction for skipping in a warc file.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum WarcSkipInstruction {
    Single {
        /// The associated skip ponter
        pointer: WarcSkipPointerWithPath,
        /// The number of octets in the body for the header signature
        header_signature_octet_count: u32,
        /// Base64 marker
        is_base64: bool
    },
    Multiple {
        /// All skip pointers, sorted in continuation order
        pointers: Vec<WarcSkipPointerWithPath>,
        /// The number of octets in the first pointer
        header_signature_octet_count: u32,
        /// Base64 marker
        is_base64: bool
    }
}

impl WarcSkipInstruction {
    pub fn new_single(pointer: WarcSkipPointerWithPath, header_signature_octet_count: u32, is_base64: bool) -> Self {
        Self::Single {
            pointer,
            header_signature_octet_count,
            is_base64
        }
    }

    pub fn new_multi(pointers: Vec<WarcSkipPointerWithPath>, header_signature_octet_count: u32, is_base64: bool) -> Self {
        Self::Multiple {
            pointers,
            header_signature_octet_count,
            is_base64
        }
    }

    /// Reads this in the context of [file_owner].
    pub async fn read_in_context(&self, file_owner: &impl FileOwner) -> Result<Option<Vec<u8>>, WarcReadError> {
        match self {
            value @ WarcSkipInstruction::Single { pointer, .. } => {
                file_owner.wait_until_free_path(pointer.path()).await?;
                value.read()
            }
            value @ WarcSkipInstruction::Multiple { pointers, .. } => {
                for value in pointers {
                    file_owner.wait_until_free_path(value.path()).await?;
                }
                value.read()
            }
        }
    }

    /// Reads this from the pointer.
    pub fn read(&self) -> Result<Option<Vec<u8>>, WarcReadError> {
        fn read_impl(pointer: &WarcSkipPointerWithPath, header_signature_octet_count: u32) -> Result<Option<Vec<u8>>, ErrorWithPath> {
            let mut file = File::options().read(true).open(pointer.path()).to_error_with_path(pointer.path())?;
            return read_body(&mut file, pointer.pointer(), header_signature_octet_count).to_error_with_path(pointer.path())
        }

        match self {
            WarcSkipInstruction::Single { pointer, header_signature_octet_count, is_base64 } => {
                let data = read_impl(pointer, *header_signature_octet_count)?;
                Ok(
                    if *is_base64 {
                        if let Some(value) = data {
                            Some(BASE64.decode(&value)?)
                        } else {
                            None
                        }
                    } else {
                        data
                    }
                )
            }
            WarcSkipInstruction::Multiple { pointers, header_signature_octet_count, is_base64 } => {
                let mut collected_data = Vec::new();
                for (pos, value) in pointers.iter().with_position() {
                    match pos {
                        Position::First | Position::Only => {
                            match read_impl(value, *header_signature_octet_count)? {
                                None => {}
                                Some(value) => {
                                    collected_data.extend(value)
                                }
                            }
                        }
                        _ => {
                            match read_impl(value, 0)? {
                                None => {}
                                Some(value) => {
                                    collected_data.extend(value)
                                }
                            }
                        }
                    }

                }
                if collected_data.is_empty() {
                    Ok(None)
                } else {
                    let collected_data = if *is_base64 {
                        BASE64.decode(&collected_data)?
                    } else {
                        collected_data
                    };
                    Ok(Some(collected_data))
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum WarcReadError {
    #[error(transparent)]
    IO(#[from] ErrorWithPath),
    #[error(transparent)]
    Decode(#[from] DecodeError),
}


macro_rules! log_consume {
    ($e: expr) => {
        {
            log::trace!(stringify!($e))
        }
        match $e {
            Ok(_) => {}
            Err(err) => {
                const ERR_HINT: &str = stringify!($e);
                log::error!("Error at {ERR_HINT}: {err}");
            }
        }
    };
}

/// Packs the header
fn pack_header(page: &CrawlResult) -> Vec<u8> {
    log::trace!("Pack header");
    let mut output = Vec::new();
    // todo: Different rest requests?
    output.extend(b"GET ");
    output.extend(page.meta.status_code.as_str().as_bytes());
    if let Some(reason) = page.meta.status_code.canonical_reason() {
        output.extend(b" ");
        output.extend(reason.as_bytes());
    }
    output.extend(b"\r\n");
    if let Some(headers) = &page.meta.headers {
        for (k, v) in headers {
            output.extend(k.as_str().as_bytes());
            output.extend(b": ");
            output.extend(v.as_bytes());
            output.extend(b"\r\n");
        }
    }
    output.extend(b"\r\n");
    log::trace!("Finished packing header");
    output
}

/// Creates a war entry
pub fn write_warc<W: SpecialWarcWriter>(worker_warc_writer: &mut W, content: &CrawlResult) -> Result<WarcSkipInstruction, DatabaseError> {
    let mut builder = WarcHeader::new();
    log_consume!(builder.warc_type(WarcRecordType::Response));
    let first_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, (&content.meta.url).as_str().as_bytes()).as_urn().to_string();
    log_consume!(builder.warc_record_id_string(first_id.clone()));
    log_consume!(builder.date(content.meta.created_at));

    if let Some(enc) = content.meta.recognized_encoding {
        log_consume!(builder.atra_content_encoding(enc));
    }

    if let Some(ref redir) = content.meta.final_redirect_destination {
        let urilike = unsafe{UriLikeFieldValue::from_string_unchecked(redir)};
        log_consume!(builder.target_uri(urilike));
    } else {
        let urilike_page = unsafe{UriLikeFieldValue::from_string_unchecked(content.meta.url.as_str())};
        log_consume!(builder.target_uri(urilike_page));
    }

    let found_ll = if let Some(ref found) = content.meta.headers {
        if let Some(found) = found.get(CONTENT_TYPE) {
            if let Ok(enc) = found.to_str() {
                Some(enc.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let found = if let Some(ref found) = found_ll {
        match parse_media_type::<true>(found.as_bytes()) {
            Ok(value) => {
                Some(value.1)
            }
            Err(err) => {
                log::error!("Failed to parse media type: {err}");
                None
            }
        }
    } else {
        None
    }.unwrap_or_else(|| {
        match &content.meta.file_information.format {
            AtraSupportedFileFormat::HTML => { MediaType::new("text", "html", None) }
            AtraSupportedFileFormat::PDF => {MediaType::new("application", "pdf", None) }
            AtraSupportedFileFormat::JavaScript => {MediaType::new("text", "javascript", None)}
            AtraSupportedFileFormat::PlainText => {MediaType::new("text", "plain", None)}
            AtraSupportedFileFormat::JSON => {MediaType::new("application", "json", None)}
            AtraSupportedFileFormat::XML => {MediaType::new("application", "xml", None)}
            AtraSupportedFileFormat::Decodeable => {MediaType::new("application", "octet-stream", None)}
            AtraSupportedFileFormat::Unknown => {MediaType::new("application", "octet-stream", None)}
        }
    });



    log_consume!(builder.content_type(found));

    let header = pack_header(&content);
    let header_signature_octet_count = header.len();

    let data = match &content.content {
        VecDataHolder::ExternalFile { file } => {
            log::trace!("Warc-Write: External");
            let (skip_pointer_path, position) = worker_warc_writer.get_skip_pointer()?;
            log_consume!(builder.external_bin_file_string(file.file_name().unwrap()));
            log_consume!(builder.content_length(header_signature_octet_count as u64));
            log_consume!(builder.header_length(header_signature_octet_count as u64));
            log_consume!(builder.truncated_reason(TruncatedReason::Length));
            let warc_header_offset = worker_warc_writer.write_header(builder)?;
            worker_warc_writer.write_body_complete(&header)?;
            return Ok(WarcSkipInstruction::new_single(
                WarcSkipPointerWithPath::create(
                    skip_pointer_path,
                    position,
                    warc_header_offset as u32,
                    header_signature_octet_count as u64
                ),
                header_signature_octet_count as u32,
                false
            ))
        }
        VecDataHolder::None => {
            log::trace!("Warc-Write: No Payload");
            let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
            log_consume!(builder.content_length(header_signature_octet_count as u64));
            log_consume!(builder.header_length(header_signature_octet_count as u64));
            let warc_header_offset = worker_warc_writer.write_header(builder)?;
            worker_warc_writer.write_body_complete(&header)?;
            return Ok(WarcSkipInstruction::new_single(
                WarcSkipPointerWithPath::create(
                    skip_pointer_path,
                    skip_position,
                    warc_header_offset as u32,
                    header_signature_octet_count as u64
                ),
                header_signature_octet_count as u32,
                false
            ))
        }

        VecDataHolder::InMemory { data } => {
            if data.is_empty() {
                log::warn!("Warc-Write: No Payload, but was detected as payload. Falling back!");
                let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
                log_consume!(builder.content_length(header_signature_octet_count as u64));
                log_consume!(builder.header_length(header_signature_octet_count as u64));
                let warc_header_offset = worker_warc_writer.write_header(builder)?;
                worker_warc_writer.write_body_complete(&header)?;
                return Ok(WarcSkipInstruction::new_single(
                    WarcSkipPointerWithPath::create(
                        skip_pointer_path,
                        skip_position,
                        warc_header_offset as u32,
                        header_signature_octet_count as u64
                    ),
                    header_signature_octet_count as u32,
                    false
                ))
            } else {
                //todo: Base64
                data
            }
        }
    };


    let mut body = header;

    let (data, is_base64) = match content.meta.file_information.format {
        AtraSupportedFileFormat::Unknown => {
            log_consume!(builder.atra_is_base64(true));
            (Cow::Owned(BASE64.encode(data.as_slice()).into_bytes()), true)
        }
        _ => (Cow::Borrowed(data.as_slice()), false)
    };

    body.extend_from_slice(&data);
    let digest = labeled_xxh128_digest(&body);

    log::trace!("Warc: Decide if multi or single");
    if data.len() > 1.gigabytes() {
        log::trace!("Warc chunk mode!");
        let mut skip_pointers = Vec::new();
        log_consume!(builder.payload_digest_bytes(digest));
        for (position, (idx, value)) in body.chunks(1.gigabytes().as_u64() as usize).enumerate().with_position() {
            let mut sub_builder = builder.clone();
            match position {
                Position::First => {
                    // warc_type set beforehand
                    log_consume!(sub_builder.header_length(header_signature_octet_count as u64));
                }
                Position::Middle => {
                    log_consume!(sub_builder.warc_record_id_string(Uuid::new_v4().as_urn().to_string()));
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                }
                Position::Last => {
                    log_consume!(sub_builder.warc_record_id_string(Uuid::new_v4().as_urn().to_string()));
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                    log_consume!(sub_builder.segment_total_length(body.len() as u64));
                }
                Position::Only => {
                    // Combination of first and last
                    log_consume!(sub_builder.header_length(header_signature_octet_count as u64));
                    log_consume!(sub_builder.warc_record_id_string(Uuid::new_v4().as_urn().to_string()));
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                    log_consume!(sub_builder.segment_total_length(body.len() as u64));
                }
            }

            log_consume!(sub_builder.block_digest_bytes(labeled_xxh128_digest(value)));
            log_consume!(sub_builder.segment_number((idx + 1) as u64));
            log_consume!(sub_builder.segment_origin_id_string(first_id.clone()));
            let content_length = value.len() as u64;
            log_consume!(sub_builder.content_length(content_length));
            let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
            let warc_header_offset = worker_warc_writer.write_header(sub_builder)?;
            worker_warc_writer.write_body_complete(&value)?;
            skip_pointers.push(
                WarcSkipPointerWithPath::create(
                    skip_pointer_path,
                    skip_position,
                    warc_header_offset as u32,
                    content_length
                )
            );
            let _ = worker_warc_writer.forward_if_filesize(1.gigabytes().as_u64() as usize);
        }
        Ok(WarcSkipInstruction::new_multi(skip_pointers, header_signature_octet_count as u32, is_base64))
    } else {
        log::trace!("Warc normal mode!");
        log_consume!(builder.header_length(header_signature_octet_count as u64));
        log_consume!(builder.block_digest_bytes(digest.clone()));
        log_consume!(builder.payload_digest_bytes(digest));
        log_consume!(builder.content_length(body.len() as u64));
        let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
        let warc_header_offset = worker_warc_writer.write_header(builder)?;
        worker_warc_writer.write_body_complete(&body)?;
        worker_warc_writer.forward_if_filesize(1.gigabytes().as_u64() as usize)?;
        return Ok(WarcSkipInstruction::new_single(
            WarcSkipPointerWithPath::create(
                skip_pointer_path,
                skip_position,
                warc_header_offset as u32,
                body.len() as u64
            ),
            header_signature_octet_count as u32,
            is_base64
        ))
    }
}


/// Reads the body from [reader] for a provided [pointer]
pub fn read_body<R: Seek + Read>(reader: &mut R, pointer: &WarcSkipPointer, header_octet_count: u32) -> Result<Option<Vec<u8>>, Error> {
    let header_octet_count = header_octet_count as u64;
    reader.seek(SeekFrom::Start(pointer.position() + pointer.warc_header_offset() as u64 + header_octet_count))?;
    let mut to_read = (pointer.body_octet_count() - header_octet_count) as usize;
    if to_read == 0 {
        return Ok(None)
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


#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;
    use reqwest::StatusCode;
    use time::OffsetDateTime;
    use encoding_rs;
    use crate::core::crawl::result::CrawlResult;
    use crate::core::fetching::FetchedRequestData;
    use crate::core::response::ResponseData;
    use crate::core::{UrlWithDepth, VecDataHolder};
    use crate::core::format::AtraFileInformation;
    use crate::core::format::mime::{MimeDescriptor, TypedMime};
    use crate::core::format::mime_typing::MimeType;
    use crate::core::format::supported::AtraSupportedFileFormat;
    use crate::core::warc::{MockSpecialWarcWriter, write_warc};

    #[test]
    fn can_write_html(){
        const HTML_DATA: &str = "<html><body>Hello World!</body></html>";
        let result = CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(
                FetchedRequestData::new(
                    VecDataHolder::from_vec(HTML_DATA.as_bytes().to_vec()),
                    None,
                    StatusCode::OK,
                    None,
                    None,
                    false
                ),
                UrlWithDepth::from_seed("https://www.google.de/0").unwrap(),
            ),
            None,
            Some(encoding_rs::UTF_8),
            AtraFileInformation::new(
                AtraSupportedFileFormat::HTML,
                MimeDescriptor::Single(TypedMime(MimeType::HTML, mime::TEXT_HTML_UTF_8)),
                None
            )
        );

        let mut special = MockSpecialWarcWriter::new();

        special.expect_get_skip_pointer().returning(|| {
            Ok(
                (
                    Utf8PathBuf::new(),
                    0
                )
            )
        });

        special.expect_write_header().return_once(
            |value| {
                let value = value.to_string();
                println!("Header:\n{value}");
                Ok(value.len())
            }
        );

        special.expect_write_body_complete().return_once(
            |value| {
                println!(
                    "Body:\n{}",
                    String::from_utf8_lossy(value)
                );
                Ok(value.len())
            }
        );

        special.expect_forward_if_filesize().returning(|_| Ok(None));


        let instruction = write_warc(&mut special, &result).expect("Should work!");

        println!("{instruction:?}")
    }





    #[test]
    fn can_write_base64(){
        const HTML_DATA: &str = "<html><body>Hello World! WARBLGARBL</body></html>";
        let result = CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(
                FetchedRequestData::new(
                    VecDataHolder::from_vec(HTML_DATA.as_bytes().to_vec()),
                    None,
                    StatusCode::OK,
                    None,
                    None,
                    false
                ),
                UrlWithDepth::from_seed("https://www.google.de/0").unwrap(),
            ),
            None,
            Some(encoding_rs::UTF_8),
            AtraFileInformation::new(
                AtraSupportedFileFormat::Unknown,
                MimeDescriptor::Empty,
                None
            )
        );

        let mut special = MockSpecialWarcWriter::new();

        special.expect_get_skip_pointer().returning(|| {
            Ok(
                (
                    Utf8PathBuf::new(),
                    0
                )
            )
        });

        special.expect_write_header().return_once(
            |value| {
                let value = value.to_string();
                println!("Header:\n{value}");
                Ok(value.len())
            }
        );

        special.expect_write_body_complete().return_once(
            |value| {
                println!(
                    "Body:\n{}",
                    String::from_utf8_lossy(value)
                );
                Ok(value.len())
            }
        );

        special.expect_forward_if_filesize().returning(|_| Ok(None));

        let instruction = write_warc(&mut special, &result).expect("Should work!");

        println!("{instruction:?}")
    }
}