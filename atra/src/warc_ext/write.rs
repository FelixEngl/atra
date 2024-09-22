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

use crate::crawl::CrawlResult;
use crate::data::RawVecData;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::toolkit::digest::labeled_xxh128_digest;
use crate::warc_ext::errors::WriterError;
use crate::warc_ext::instructions::WarcSkipInstruction;
use crate::warc_ext::skip_pointer::WarcSkipPointerWithPath;
use crate::warc_ext::special_writer::SpecialWarcWriter;
use data_encoding::BASE64;
use itertools::{Itertools, Position};
use reqwest::header::CONTENT_TYPE;
use std::borrow::Cow;
use ubyte::ToByteUnit;
use uuid::Uuid;
use warc::field::UriLikeFieldValue;
use warc::header::WarcHeader;
use warc::media_type::parse_media_type;
use warc::record_type::WarcRecordType;
use warc::truncated_reason::TruncatedReason;

macro_rules! log_consume {
    ($e: expr) => {{
        log::trace!(stringify!($e))
    }
    match $e {
        Ok(_) => {}
        Err(err) => {
            const ERR_HINT: &str = stringify!($e);
            log::error!("Error at {ERR_HINT}: {err}");
        }
    }};
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
pub fn write_warc<W: SpecialWarcWriter>(
    worker_warc_writer: &mut W,
    content: &CrawlResult,
) -> Result<WarcSkipInstruction, WriterError> {
    let mut builder = WarcHeader::new();
    log_consume!(builder.warc_type(WarcRecordType::Response));
    let first_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        (&content.meta.url).try_as_str().as_bytes(),
    )
    .as_urn()
    .to_string();
    log_consume!(builder.warc_record_id_string(&first_id));
    log_consume!(builder.date(content.meta.created_at));

    if let Some(enc) = content.meta.recognized_encoding {
        log_consume!(builder.atra_content_encoding(enc));
    }

    if let Some(language) = &content.meta.language {
        log_consume!(builder.atra_language_hint(language.lang()));
    }

    if let Some(ref redir) = content.meta.final_redirect_destination {
        let urilike = unsafe { UriLikeFieldValue::from_string_unchecked(redir) };
        log_consume!(builder.target_uri(urilike));
    } else {
        let urilike_page =
            unsafe { UriLikeFieldValue::from_string_unchecked(&content.meta.url.try_as_str()) };
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
            Ok(value) => value.1,
            Err(err) => {
                log::error!("Failed to parse media type: {err}");
                content.meta.file_information.get_best_media_type_for_warc()
            }
        }
    } else {
        content.meta.file_information.get_best_media_type_for_warc()
    };

    log_consume!(builder.content_type(found));

    let header = pack_header(&content);
    let header_signature_octet_count = header.len();

    let data = match &content.content {
        RawVecData::ExternalFile { file } => {
            log::trace!("Warc-Write: External");
            let (skip_pointer_path, position) = worker_warc_writer.get_skip_pointer()?;
            log_consume!(builder.external_bin_file_string(file.file_name().unwrap()));
            log_consume!(builder.content_length(header_signature_octet_count as u64));
            log_consume!(builder.atra_header_length(header_signature_octet_count as u64));
            log_consume!(builder.truncated_reason(TruncatedReason::Length));
            let warc_header_offset = worker_warc_writer.write_header(builder)?;
            worker_warc_writer.write_body_complete(&header)?;
            return Ok(WarcSkipInstruction::new_single(
                WarcSkipPointerWithPath::create(
                    skip_pointer_path,
                    position,
                    warc_header_offset as u32,
                    header_signature_octet_count as u64,
                ),
                header_signature_octet_count as u32,
                false,
            ));
        }
        RawVecData::None => {
            log::trace!("Warc-Write: No Payload");
            let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
            log_consume!(builder.content_length(header_signature_octet_count as u64));
            log_consume!(builder.atra_header_length(header_signature_octet_count as u64));
            let warc_header_offset = worker_warc_writer.write_header(builder)?;
            worker_warc_writer.write_body_complete(&header)?;
            return Ok(WarcSkipInstruction::new_single(
                WarcSkipPointerWithPath::create(
                    skip_pointer_path,
                    skip_position,
                    warc_header_offset as u32,
                    header_signature_octet_count as u64,
                ),
                header_signature_octet_count as u32,
                false,
            ));
        }

        RawVecData::InMemory { data } => {
            if data.is_empty() {
                log::warn!("Warc-Write: No Payload, but was detected as payload. Falling back!");
                let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
                log_consume!(builder.content_length(header_signature_octet_count as u64));
                log_consume!(builder.atra_header_length(header_signature_octet_count as u64));
                let warc_header_offset = worker_warc_writer.write_header(builder)?;
                worker_warc_writer.write_body_complete(&header)?;
                return Ok(WarcSkipInstruction::new_single(
                    WarcSkipPointerWithPath::create(
                        skip_pointer_path,
                        skip_position,
                        warc_header_offset as u32,
                        header_signature_octet_count as u64,
                    ),
                    header_signature_octet_count as u32,
                    false,
                ));
            } else {
                //todo: Base64
                data
            }
        }
    };

    let mut body = header;

    let (data, is_base64) = match content.meta.file_information.format {
        InterpretedProcessibleFileFormat::Unknown => {
            log_consume!(builder.atra_is_base64(true));
            (
                Cow::Owned(BASE64.encode(data.as_slice()).into_bytes()),
                true,
            )
        }
        _ => (Cow::Borrowed(data.as_slice()), false),
    };

    body.extend_from_slice(&data);
    let digest = labeled_xxh128_digest(&body);

    log::trace!("Warc: Decide if multi or single");
    if data.len() > 1.gigabytes() {
        log::trace!("Warc chunk mode!");
        let mut skip_pointers = Vec::new();
        log_consume!(builder.payload_digest_bytes(digest));
        for (position, (idx, value)) in body
            .chunks(1.gigabytes().as_u64() as usize)
            .enumerate()
            .with_position()
        {
            let mut sub_builder = builder.clone();
            match position {
                Position::First => {
                    // warc_type set beforehand
                    log_consume!(
                        sub_builder.atra_header_length(header_signature_octet_count as u64)
                    );
                }
                Position::Middle => {
                    log_consume!(
                        sub_builder.warc_record_id_string(&Uuid::new_v4().as_urn().to_string())
                    );
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                }
                Position::Last => {
                    log_consume!(
                        sub_builder.warc_record_id_string(&Uuid::new_v4().as_urn().to_string())
                    );
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                    log_consume!(sub_builder.segment_total_length(body.len() as u64));
                }
                Position::Only => {
                    // Combination of first and last
                    log_consume!(
                        sub_builder.atra_header_length(header_signature_octet_count as u64)
                    );
                    log_consume!(
                        sub_builder.warc_record_id_string(&Uuid::new_v4().as_urn().to_string())
                    );
                    log_consume!(sub_builder.warc_type(WarcRecordType::Continuation));
                    log_consume!(sub_builder.segment_total_length(body.len() as u64));
                }
            }

            log_consume!(sub_builder.block_digest_bytes(labeled_xxh128_digest(value)));
            log_consume!(sub_builder.segment_number((idx + 1) as u64));
            log_consume!(sub_builder.segment_origin_id_string(&first_id));
            let content_length = value.len() as u64;
            log_consume!(sub_builder.content_length(content_length));
            let (skip_pointer_path, skip_position) = worker_warc_writer.get_skip_pointer()?;
            let warc_header_offset = worker_warc_writer.write_header(sub_builder)?;
            worker_warc_writer.write_body_complete(&value)?;
            skip_pointers.push(WarcSkipPointerWithPath::create(
                skip_pointer_path,
                skip_position,
                warc_header_offset as u32,
                content_length,
            ));
            let _ = worker_warc_writer.forward_if_filesize(1.gigabytes().as_u64() as usize);
        }
        Ok(WarcSkipInstruction::new_multi(
            skip_pointers,
            header_signature_octet_count as u32,
            is_base64,
        ))
    } else {
        log::trace!("Warc normal mode!");
        log_consume!(builder.atra_header_length(header_signature_octet_count as u64));
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
                body.len() as u64,
            ),
            header_signature_octet_count as u32,
            is_base64,
        ));
    }
}
