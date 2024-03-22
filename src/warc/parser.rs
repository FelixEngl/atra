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

use std::str::{FromStr};

use nom::bytes::streaming::tag;
use nom::character::streaming::digit1;
use nom::combinator::{map_res, not, verify};
use nom::{IResult};
use nom::multi::many1;
use nom::sequence::{delimited, separated_pair, terminated};
use strum::EnumString;

use crate::warc::field::{WarcFieldName, WarcFieldValue};
use crate::warc::header::WarcHeader;

/// The parse method when reading a
fn parse_warc_header_name(data: &[u8]) -> IResult<&[u8], WarcFieldName> {
    return map_res(
        terminated(
            verify(
                nom::bytes::streaming::take_till1(|c| c == b':'),
                |value: &[u8]| !value.contains(&b'\r')
            ),
            tag(b":")
        ),
        |value| {
            let interpreted = String::from_utf8_lossy(value);
            WarcFieldName::from_str(&interpreted)
        }
    )(data)
}

fn parse_warc_header_entry(data: &[u8]) -> IResult<&[u8], (WarcFieldName, WarcFieldValue)> {
    // We peek for a plain old \r\n as the first two bytes,t this indicates, that we are done.
    not(tag(b"\r\n"))(data)?;
    let (data, header) = parse_warc_header_name(data)?;
    let (data, selected) =
        terminated(
            map_res(
                nom::bytes::streaming::take_till1(|c| c == b'\r'),
                |value| { WarcFieldValue::parse(&header, value) }
            ),
            tag(b"\r\n")
        )(data)?;
    Ok((data, (header, selected)))
}

const WARC_START: &[u8; 5] = b"WARC/";

fn parse_warc_version_raw(b: &[u8]) -> IResult<&[u8], (&[u8], &[u8])> {
    delimited(
        tag(WARC_START),
        separated_pair(
            digit1,
            tag(b"."),
            digit1
        ),
        tag(b"\r\n")
    )(b)
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, EnumString)]
pub enum WarcVersionPeek {
    NotEnoughBytes,
    NotFound,
    /// If more data is needed, bool is true
    StartsCorrectly(bool),
    /// If more data is needed, bool is true
    FirstDigit(bool),
    /// If more data is needed, bool is true
    Dot(bool),
    Complete,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum SimpleState {
    ScanDigit1,
    Dot,
    ScanDigit2,
    Finished,
}

/// Peeks the warc version
pub fn peek_warc_version(b: &[u8]) -> WarcVersionPeek {
    if b.len() < 5 {
        return WarcVersionPeek::NotEnoughBytes
    }
    match b.strip_prefix(WARC_START) {
        None => {WarcVersionPeek::NotFound}
        Some(found) => {
            if found.is_empty() {
                WarcVersionPeek::StartsCorrectly(true)
            } else {
                let mut state = SimpleState::ScanDigit1;
                let mut it = found.into_iter().copied();
                while let Some(u) = it.next() {
                    match state {
                        SimpleState::ScanDigit1 => {
                            if u.is_ascii_digit() {
                                continue
                            }
                            if u == b'.' {
                                state = SimpleState::Dot;
                                continue
                            }
                            break
                        }
                        SimpleState::Dot => {
                            state = SimpleState::ScanDigit2;
                        }
                        SimpleState::ScanDigit2 => {
                            if u.is_ascii_digit() {
                                continue
                            }
                            state = SimpleState::Finished;
                            break
                        }
                        SimpleState::Finished => {
                            unreachable!()
                        }
                    }
                }

                match state {
                    SimpleState::ScanDigit1 => {
                        WarcVersionPeek::StartsCorrectly(it.next().is_none())
                    }
                    SimpleState::Dot => {
                        WarcVersionPeek::FirstDigit(it.next().is_none())
                    }
                    SimpleState::ScanDigit2 => {
                        WarcVersionPeek::Dot(it.next().is_none())
                    }
                    SimpleState::Finished => {WarcVersionPeek::Complete}
                }
            }
        }
    }
}

pub fn parse_warc_version(b: &[u8]) -> IResult<&[u8], String>
{
    map_res(
        parse_warc_version_raw,
        |(a, b)| {
            match std::str::from_utf8(a) {
                Ok(a) => {
                    match std::str::from_utf8(b) {
                        Ok(b) => {
                            Ok(
                                format!(
                                    "WARC/{}.{}",
                                    a,
                                    b,
                                )
                            )
                        }
                        Err(err) => {
                            Err(err)
                        }
                    }
                }
                Err(err) => {
                    Err(err)
                }
            }

        }
    )(b)
}

/// Parses a warc header.
pub fn parse_warc_header(b: &[u8]) -> IResult<&[u8], WarcHeader> {
    let (b, version) = parse_warc_version(b)?;

    let (b, data) = terminated(
        many1(parse_warc_header_entry),
        tag(b"\r\n")
    )(b)?;

    let mut header = WarcHeader::with_version(version);
    unsafe {
        for (k, v) in data.into_iter() {
            header.unchecked_field(k, v);
        }
    }
    Ok((b, header))
}

#[cfg(test)]
pub(crate) mod test {
    use std::net::{IpAddr, Ipv4Addr};

    use encoding_rs::UTF_8;
    use time::OffsetDateTime;
    use crate::warc::field::{GeneralFieldValue, UriLikeFieldValue};

    use crate::warc::media_type::parse_media_type;
    use crate::warc::parser::{parse_warc_header, peek_warc_version};
    use crate::warc::header::{WarcHeader};
    use crate::warc::record_type::WarcRecordType;
    use crate::warc::truncated_reason::TruncatedReason;

    fn create_uri_num(id_base: &str, ct: u64) -> UriLikeFieldValue {
        UriLikeFieldValue::new(GeneralFieldValue::from_string(format!("https://www.{id_base}.com/{ct}")).unwrap()).unwrap()
    }

    pub fn create_test_header(id_base: &str, content_length: u64) -> WarcHeader {
        let mut data = WarcHeader::new();
        let mut uri_ct = 0;
        data.warc_record_id(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.concurrent_to(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.refers_to(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.refers_to_target(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.target_uri(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.info_id(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.profile(create_uri_num(id_base, {let x = uri_ct; uri_ct+=1; x})).unwrap();
        data.segment_origin_id(create_uri_num(id_base, uri_ct)).unwrap();

        data.warc_type(WarcRecordType::Response).unwrap();

        data.atra_content_encoding(UTF_8).unwrap();

        data.date(OffsetDateTime::now_utc()).unwrap();
        data.referes_to_date(OffsetDateTime::now_utc()).unwrap();

        data.content_length(content_length).unwrap();
        data.segment_number(1234).unwrap();
        data.segment_total_length(12345).unwrap();

        data.content_type(parse_media_type::<true>(b"text/html;charset=UTF-8").unwrap().1).unwrap();
        data.indentified_payload_type(parse_media_type::<true>(b"text/xml").unwrap().1).unwrap();

        data.truncated_reason(TruncatedReason::Length).unwrap();

        data.ip_address(IpAddr::V4(Ipv4Addr::new(127,0,0,1))).unwrap();

        data.block_digest_string("sha1:bla".to_string()).unwrap();
        data.payload_digest_string("sha1:bla".to_string()).unwrap();

        data.file_name_string("lolwut.txt".to_string()).unwrap();

        data
    }

    #[test]
    fn test_header_parser(){
        let data = create_test_header("google", 123);
        let mut x = Vec::new();
        data.write_to(&mut x, true).unwrap();
        println!(
            "{:?}",
            data
        );
        println!("----");
        println!("{}", String::from_utf8(x.clone()).unwrap());
        println!("----");
        println!(
            "{:?}",
            parse_warc_header(&x).unwrap().1
        )
    }

    #[test]
    fn test_peeking() {
        let header = create_test_header("hellofresh", 123);
        let mut buf = Vec::new();
        header.write_to(&mut buf, true).unwrap();
        buf.extend_from_slice(b"lol\r\n\r\n".as_slice());
        println!("{}", String::from_utf8(buf.clone()).unwrap().escape_debug());
        println!("{:?}", peek_warc_version(&buf[..4]));
        println!("{:?}", peek_warc_version(&buf[..5]));
        println!("{:?}", peek_warc_version(&buf[..6]));
        println!("{:?}", peek_warc_version(&buf[..7]));
        println!("{:?}", peek_warc_version(&buf[..8]));
        println!("{:?}", peek_warc_version(&buf[..9]));
        println!("{:?}", peek_warc_version(&buf[..10]));
    }
}