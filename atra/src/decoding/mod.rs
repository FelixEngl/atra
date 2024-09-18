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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::{Decoded, RawData, RawVecData};
use crate::fetching::ResponseData;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::AtraFileInformation;
use crate::io::fs::AtraFS;
use crate::static_selectors;
use camino::Utf8PathBuf;
use chardetng::EncodingDetector;
use encoding_rs::{DecoderResult, Encoding, UTF_8};
use itertools::Itertools;
use scraper::Html;
use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Write};
use thiserror::Error;
use tokio::task::yield_now;

/// An error while decoding
#[derive(Debug, Error)]
pub enum DecodingError {
    #[error(transparent)]
    IO(#[from] io::Error),
    #[error("Decoding the big file failed somehow!")]
    DecodingFileFailed,
    #[error("Out of disc memory!")]
    OutOfMemory,
}

/// Decode complete input to `Cow<'a, str>` _with BOM sniffing_ and with
/// malformed sequences replaced with the REPLACEMENT CHARACTER when the
/// entire input is available as a single buffer (i.e. the end of the
/// buffer marks the end of the stream).
///
/// The BOM, if any, does not appear in the output.
///
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
pub async fn decode<'a, C>(
    context: &C,
    page: &'a ResponseData,
    identified_type: &AtraFileInformation,
) -> Result<Decoded<Cow<'a, str>, Utf8PathBuf>, DecodingError>
where
    C: SupportsConfigs + SupportsFileSystemAccess,
{
    match page.content() {
        RawVecData::None => return Ok(Decoded::None),
        RawVecData::ExternalFile { .. } => {
            if let Some(max_size) = context.configs().crawl.decode_big_files_up_to {
                let size = page.content().size()?;
                if max_size < size {
                    log::info!("Skip decoding for {} because the file has {size} bytes but the maximum is {max_size}", page.url);
                    return Ok(Decoded::None);
                }
            }
        }
        _ => {}
    }

    let mut decodings = get_decoders_by_mime(identified_type).unwrap_or_default();

    // use probably defective encodings from header and body somewhere?
    if identified_type.format == InterpretedProcessibleFileFormat::HTML {
        static_selectors! {
            [
                META_CHARSET = "meta[charset]"
            ]
        }

        if let Some(content) = page.content.as_in_memory() {
            let lossy_parsed =
                Html::parse_document(String::from_utf8_lossy(content.as_slice()).as_ref());
            let found_in_html: Option<Vec<&'static Encoding>> = lossy_parsed
                .select(&META_CHARSET)
                .filter_map(|value| {
                    value
                        .attr("charset")
                        .map(|value| Encoding::for_label_no_replacement(value.as_bytes()))
                })
                .collect();

            if let Some(found) = found_in_html {
                decodings.extend(found);
            }
        }
    }

    for enc in decodings.iter() {
        let succ = do_decode(page, *enc)?;
        match &succ {
            Decoded::InMemory {
                encoding,
                had_errors,
                ..
            } => {
                if *had_errors {
                    log::debug!("Failed to decode {} with {}.", page.url, encoding.name());
                    continue;
                }
            }
            Decoded::OffMemory {
                reference: result,
                encoding,
                had_errors,
            } => {
                if *had_errors {
                    log::debug!("Failed to decode {} with {}.", page.url, encoding.name());
                    context.fs().cleanup_data_file(result.as_str())?;
                    continue;
                }
            }
            Decoded::None => {
                continue;
            }
        }
        return Ok(succ);
    }

    yield_now().await;

    decode_by_bom(context, page)
}

fn get_decoders_by_mime<'a>(
    identified_type: &AtraFileInformation,
) -> Option<Vec<&'static Encoding>> {
    let mime = identified_type.mime.as_ref()?;
    if let Some(param) = mime.get_param_values(mime::CHARSET) {
        if param.is_empty() {
            None
        } else {
            Some(
                param
                    .iter()
                    .filter_map(|value| Encoding::for_label(value.as_str().as_bytes()))
                    .collect_vec(),
            )
        }
    } else {
        None
    }
}

/// Decodes by BOM only.
fn decode_by_bom<'a, C>(
    context: &C,
    page: &'a ResponseData,
) -> Result<Decoded<Cow<'a, str>, Utf8PathBuf>, DecodingError>
where
    C: SupportsFileSystemAccess,
{
    let bom_buf = page.content().peek_bom(context)?;

    if let Some((encoder, _)) = Encoding::for_bom(&bom_buf) {
        do_decode(page, encoder)
    } else {
        let mut enc = EncodingDetector::new();

        let result = match page.content() {
            RawVecData::InMemory { data } => enc.feed(data.as_ref(), true),
            RawVecData::ExternalFile { file } => {
                let mut reader = BufReader::new(File::options().read(true).open(file)?);
                let mut has_non_ascii = false;
                loop {
                    let buf = reader.fill_buf()?;
                    if buf.is_empty() {
                        break;
                    }
                    if enc.feed(buf, false) {
                        has_non_ascii = true
                    }
                    let needed = buf.len();
                    reader.consume(needed);
                }
                has_non_ascii
            }
            RawVecData::None => unreachable!(),
        };

        if result {
            let domain = page.get_url_parsed().domain();
            let domain = domain
                .as_ref()
                .map(|value| psl::domain(value.as_bytes()))
                .flatten();
            let (selected_encoding, is_probably_right) = if let Some(domain) = domain {
                enc.guess_assess(Some(domain.suffix().as_bytes()), false)
            } else {
                enc.guess_assess(None, false)
            };
            if is_probably_right {
                let result = do_decode(page, selected_encoding)?;
                if result.had_errors() {
                    let try_utf8 = do_decode(page, UTF_8)?;
                    if try_utf8.had_errors() {
                        Ok(result)
                    } else {
                        Ok(try_utf8)
                    }
                } else {
                    Ok(result)
                }
            } else {
                do_decode(page, UTF_8)
            }
        } else {
            do_decode(page, UTF_8)
        }
    }
}

/// Decodes the content of [page] with [encoding]
fn do_decode<'a>(
    page: &'a ResponseData,
    encoding: &'static Encoding,
) -> Result<Decoded<Cow<'a, str>, Utf8PathBuf>, DecodingError> {
    match &page.content {
        RawData::InMemory { data } => {
            let decoded = encoding.decode(data.as_slice());
            if decoded.2 {
                log::info!(
                    "The page for {} had an error while decoding with {}. ",
                    page.url,
                    encoding.name()
                );
            }
            Ok(decoded.into())
        }
        RawData::ExternalFile { file } => {
            let mut decoder = encoding.new_decoder_with_bom_removal();
            let mut out_path = file.clone();
            {
                let mut name = out_path
                    .file_name()
                    .expect("A DataFile always has a name!")
                    .to_string();
                name.push_str("_decoded_");
                name.push_str(encoding.name());
                out_path.set_file_name(name);
            }
            let mut output = File::options().write(true).open(&out_path)?;
            let mut reader = BufReader::new(File::options().read(true).open(&file)?);

            // Bare metal platforms usually have very small amounts of RAM
            // (in the order of hundreds of KB)
            pub const DEFAULT_BUF_SIZE: usize = if cfg!(target_os = "espidf") {
                512
            } else {
                8 * 1024
            };

            let mut output_buf = [0u8; DEFAULT_BUF_SIZE];
            let mut had_error = false;
            loop {
                let buffer = reader.fill_buf()?;
                if buffer.is_empty() {
                    let (result, read, written) =
                        decoder.decode_to_utf8_without_replacement(buffer, &mut output_buf, true);
                    assert_eq!(
                        DecoderResult::InputEmpty,
                        result,
                        "The input should be empty!"
                    );
                    assert_eq!(0, read, "Read some data but shoudn't!");
                    assert_eq!(0, written, "Wrote some data but shoudn't!");
                    break;
                }
                let (result, read, written) =
                    decoder.decode_to_utf8_without_replacement(buffer, &mut output_buf, false);

                if read == 0 {
                    // Something bad happened.
                    panic!("Was not able to convert a single byte of {}, something went horribly wrong! Deactivate the big-file feature or contact the developer!", page.url);
                }

                match result {
                    DecoderResult::InputEmpty => return Err(DecodingError::DecodingFileFailed),
                    DecoderResult::OutputFull => return Err(DecodingError::OutOfMemory),
                    DecoderResult::Malformed(..) => {
                        had_error = true;
                    }
                }

                let real_written = output.write(&output_buf[..written])?;
                assert_eq!(
                    written, real_written,
                    "Expected to write {written} but only wrote {real_written}!"
                );
                reader.consume(read);
            }
            Ok(Decoded::new_off_memory(out_path, encoding, had_error))
        }
        RawData::None => unreachable!(),
    }
}

#[cfg(test)]
mod test {
    use crate::decoding::decode;
    use crate::fetching::{FetchedRequestData, ResponseData};
    use crate::format::AtraFileInformation;
    use crate::test_impls::*;
    use encoding_rs::Encoding;
    use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
    use reqwest::StatusCode;
    use std::borrow::Cow;

    macro_rules! test_for {
        (@old $name: ident: $sample: ident($encoding: expr)) => {
            #[allow(non_snake_case)]
            #[tokio::test]
            async fn $name(){
                let original_enc = $encoding;
                let (website, content) = $sample(original_enc);
                let context = TestContext::default();
                let format = AtraFileInformation::determine(&context, &website);
                let decoded = decode(&context, &website, &format).await.unwrap();
                assert_eq!(content, decoded.as_in_memory().unwrap().as_ref(), "The selected encoding {} does not equal the selected decoding {}", original_enc.name(), decoded.encoding().unwrap().name());
            }
        };

        (@modern $name: ident: $sample: ident($encoding: expr)) => {
            #[allow(non_snake_case)]
            #[tokio::test]
            async fn $name(){
                let original_enc = $encoding;
                let (website, content) = $sample(original_enc);
                let context = TestContext::default();
                let format = AtraFileInformation::determine(&context, &website);
                let decoded = decode(&context, &website, &format).await.unwrap();
                assert_eq!(original_enc, decoded.encoding().unwrap(), "The selected encoding {} does not equal the selected decoding {}", original_enc.name(), decoded.encoding().unwrap().name());
                assert_eq!(content, decoded.as_in_memory().unwrap().as_ref());
            }
        };

        ($name: ident: $sample: ident($encoding: expr)) => {
            test_for!(@modern $name: $sample($encoding));
        };

        ($name: ident($encoding: expr)) => {
            test_for!(@old $name: website_old($encoding));
        };
    }

    fn encode<'a>(
        encoding: &'static Encoding,
        data: &'a str,
    ) -> (Cow<'a, [u8]>, &'static Encoding, bool) {
        encoding.encode(data)
    }

    fn website_old(encoding: &'static Encoding) -> (ResponseData, &'static str) {
        const DATA: &'static str = include_str!("../../testdata/samples/sample_1.html");
        let (content, used_enc, _) = encode(encoding, DATA);
        assert_eq!(
            encoding,
            used_enc,
            "The used encoding {} differs from the expected one {}",
            used_enc.name(),
            encoding.name()
        );
        let data = FetchedRequestData::new(
            RawData::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false,
        );
        (
            ResponseData::new(
                data,
                UrlWithDepth::from_url("https://www.example.com").unwrap(),
            ),
            DATA,
        )
    }

    fn website_modern1(encoding: &'static Encoding) -> (ResponseData, &'static str) {
        const DATA: &'static str = include_str!("../../testdata/samples/sample_1.html");
        let (content, used_enc, _) = encode(encoding, DATA);
        assert_eq!(
            encoding,
            used_enc,
            "The used encoding {} differs from the expected one {}",
            used_enc.name(),
            encoding.name()
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::try_from(format!("text/html; charset={}", encoding.name())).unwrap(),
        );
        let data = FetchedRequestData::new(
            RawData::from_vec(content.to_vec()),
            Some(headers),
            StatusCode::OK,
            None,
            None,
            false,
        );
        (
            ResponseData::new(
                data,
                UrlWithDepth::from_url("https://www.example.com").unwrap(),
            ),
            DATA,
        )
    }

    fn website_modern2(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../../testdata/samples/sample_2.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(
            encoding,
            used_enc,
            "The used encoding {} differs from the expected one {}",
            used_enc.name(),
            encoding.name()
        );
        let data = FetchedRequestData::new(
            RawData::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false,
        );
        (
            ResponseData::new(
                data,
                UrlWithDepth::from_url("https://www.example.com").unwrap(),
            ),
            replaces,
        )
    }

    fn website_modern3(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../../testdata/samples/sample_3.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(
            encoding,
            used_enc,
            "The used encoding {} differs from the expected one {}",
            used_enc.name(),
            encoding.name()
        );
        let data = FetchedRequestData::new(
            RawData::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false,
        );
        (
            ResponseData::new(
                data,
                UrlWithDepth::from_url("https://www.example.com").unwrap(),
            ),
            replaces,
        )
    }

    fn website_modern4(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../../testdata/samples/sample_4.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(
            encoding,
            used_enc,
            "The used encoding {} differs from the expected one {}",
            used_enc.name(),
            encoding.name()
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::try_from(format!("text/html; charset={}", encoding.name())).unwrap(),
        );
        let data = FetchedRequestData::new(
            RawData::from_vec(content.to_vec()),
            Some(headers),
            StatusCode::OK,
            None,
            None,
            false,
        );
        (
            ResponseData::new(
                data,
                UrlWithDepth::from_url("https://www.example.com").unwrap(),
            ),
            replaces,
        )
    }

    use crate::data::RawData;
    use crate::url::UrlWithDepth;
    use paste::paste;

    macro_rules! multi_test_for {
        ($encoding: ident) => {
            paste! {
                test_for!([<test_ $encoding _old>](encoding_rs::$encoding));
                test_for!([<test_ $encoding _modern1>]: website_modern1(encoding_rs::$encoding));
                test_for!([<test_ $encoding _modern2>]: website_modern2(encoding_rs::$encoding));
                test_for!([<test_ $encoding _modern3>]: website_modern3(encoding_rs::$encoding));
                test_for!([<test_ $encoding _modern4>]: website_modern4(encoding_rs::$encoding));
            }
        };
    }

    multi_test_for!(UTF_8);
    // multi_test_for!(UTF_16BE);
    // multi_test_for!(UTF_16LE);
    multi_test_for!(BIG5);
    multi_test_for!(EUC_JP);
    multi_test_for!(EUC_KR);
    multi_test_for!(GB18030);
    multi_test_for!(GBK);
    multi_test_for!(IBM866);
    multi_test_for!(SHIFT_JIS);
    multi_test_for!(ISO_8859_2);
    multi_test_for!(ISO_8859_3);
    multi_test_for!(ISO_8859_4);
    multi_test_for!(ISO_8859_5);
    multi_test_for!(ISO_8859_6);
    multi_test_for!(ISO_8859_7);
    multi_test_for!(ISO_8859_8);
    multi_test_for!(ISO_8859_8_I);
    multi_test_for!(ISO_8859_10);
    multi_test_for!(ISO_8859_13);
    multi_test_for!(ISO_8859_14);
    multi_test_for!(ISO_8859_15);
    multi_test_for!(ISO_8859_16);
    multi_test_for!(ISO_2022_JP);
    multi_test_for!(WINDOWS_874);
    multi_test_for!(WINDOWS_1250);
    multi_test_for!(WINDOWS_1251);
    multi_test_for!(WINDOWS_1252);
    multi_test_for!(WINDOWS_1253);
    multi_test_for!(WINDOWS_1256);
    multi_test_for!(WINDOWS_1254);
    multi_test_for!(WINDOWS_1255);
    multi_test_for!(WINDOWS_1257);
    multi_test_for!(WINDOWS_1258);
    multi_test_for!(KOI8_R);
    multi_test_for!(KOI8_U);
    multi_test_for!(X_MAC_CYRILLIC);
}
