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

pub mod data_holder;

use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, Write};
use camino::Utf8PathBuf;
use chardetng::EncodingDetector;
use encoding_rs::{DecoderResult, Encoding, UTF_8};
use itertools::Itertools;
use scraper::Html;
use smallvec::SmallVec;
use thiserror::Error;
use tokio::task::yield_now;
pub use data_holder::DecodedData;
use crate::core::contexts::Context;
use crate::core::{DataHolder, VecDataHolder};
use crate::core::format::AtraFileInformation;
use crate::core::response::{ResponseData};
use crate::core::format::supported::{AtraSupportedFileFormat};
use crate::static_selectors;

/// An error while decoding
#[derive(Debug, Error)]
pub enum DecodingError {
    #[error(transparent)]
    IO(#[from] io::Error)
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

pub async fn decode<'a>(context: &impl Context, page: &'a ResponseData, identified_type: &AtraFileInformation) -> Result<DecodedData<Cow<'a, str>, Utf8PathBuf>, DecodingError> {

    match page.content() {
        VecDataHolder::None => {return Ok(DecodedData::None)}
        VecDataHolder::ExternalFile { .. } => {
            if let Some(max_size) = context.configs().crawl().decode_big_files_up_to {
                let size = page.content().size()?;
                if max_size < size {
                    log::info!("Skip decoding for {} because the file has {size} bytes but the maximum is {max_size}", page.url);
                    return Ok(DecodedData::None)
                }

            }
        }
        _ => {}
    }


    // use probably defective encodings from header and body somewhere?
    let _ = if identified_type.format == AtraSupportedFileFormat::HTML {
        static_selectors! {
            [
                META_CHARSET = "meta[charset]"
            ]
        }

        let decoding: SmallVec<[&'static Encoding; 8]> = if let Some(content) = page.content.as_in_memory() {
            let lossy_parsed = Html::parse_document(String::from_utf8_lossy(content.as_slice()).as_ref());
            let found_in_html:Option<Vec<&'static Encoding>> =
                lossy_parsed
                    .select(&META_CHARSET)
                    .filter_map(|value| value.attr("charset").map(|value| Encoding::for_label_no_replacement(value.as_bytes())))
                    .collect();

            if let Some(found) = found_in_html {
                found.into_iter().chain(identified_type.determine_decoding_by_mime()).unique().collect()
            } else {
                identified_type.determine_decoding_by_mime()
            }
        } else {
            identified_type.determine_decoding_by_mime()
        };


        for enc in decoding.iter() {
            let succ = do_decode(page, *enc).await?;
            match &succ {
                DecodedData::InMemory { encoding, had_errors, .. } => {
                    if *had_errors {
                        log::debug!("Failed to decode {} with {}.", page.url, encoding.name());
                        continue;
                    }
                }
                DecodedData::OffMemory { result, encoding, had_errors } => {
                    if *had_errors {
                        log::debug!("Failed to decode {} with {}.", page.url, encoding.name());
                        context.fs().cleanup_data_file(result.as_str())?;
                        continue;
                    }
                }
                DecodedData::None => {
                    continue;
                }
            }
            return Ok(succ)
        }

        if decoding.is_empty() {
            None
        } else {
            Some(decoding)
        }
    } else {
        None
    };

    yield_now().await;

    let bom_buf = page.content().peek_bom(context)?;

    if let Some((encoder, _)) = Encoding::for_bom(&bom_buf) {
        do_decode(page, encoder).await
    } else {
        let mut enc = EncodingDetector::new();

        let result = match page.content() {
            VecDataHolder::InMemory { data } => {
                enc.feed(data.as_ref(), true)
            }
            VecDataHolder::ExternalFile { file } => {
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
            VecDataHolder::None => unreachable!()
        };

        if result {
            let (selected_encoding, is_probably_right) =
                if let Some(domain) = page.get_url_parsed().domain().map(|value| psl::domain(value.as_bytes())).flatten(){
                    enc.guess_assess(Some(domain.suffix().as_bytes()), false)
                } else {
                    enc.guess_assess(None, false)
                };
            if is_probably_right {
                let result = do_decode(page, selected_encoding).await?;
                if result.had_errors() {
                    let try_utf8 = do_decode(page, UTF_8).await?;
                    if try_utf8.had_errors() {
                        Ok(result)
                    } else {
                        Ok(try_utf8)
                    }
                } else {
                    Ok(result)
                }
            } else {
                do_decode(page, UTF_8).await
            }
        } else {
            do_decode(page, UTF_8).await
        }
    }
}

/// Decodes the content of [page] with [encoding]
pub async fn do_decode<'a>(page: &'a ResponseData, encoding: &'static Encoding) -> Result<DecodedData<Cow<'a, str>, Utf8PathBuf>, DecodingError> {
    match &page.content {
        DataHolder::InMemory { data } => {
            let decoded = encoding.decode(data.as_slice());
            if decoded.2 {
                log::info!("The page for {} had an error while decoding with {}. ", page.url, encoding.name());
            }
            return Ok(decoded.into());
        }
        DataHolder::ExternalFile { file } => {
            let mut decoder = encoding.new_decoder_with_bom_removal();
            let mut out_path = file.clone();
            {
                let mut name = out_path.file_name().expect("A DataFile always has a name!").to_string();
                name.push_str("_decoded_");
                name.push_str(encoding.name());
                out_path.set_file_name(name);
            }
            let mut output = File::options().write(true).open(&out_path)?;
            let mut reader = BufReader::new(File::options().read(true).open(&file)?);
            let mut output_buf = [0u8; crate::core::DEFAULT_BUF_SIZE];
            let mut had_error = false;
            loop {
                let buffer = reader.fill_buf()?;
                if buffer.is_empty() {
                    let (result, read, written) = decoder.decode_to_utf8_without_replacement(
                        buffer,
                        &mut output_buf,
                        true
                    );
                    assert_eq!(DecoderResult::InputEmpty, result, "The input should be empty!");
                    assert_eq!(0, read, "Read some data but shoudn't!");
                    assert_eq!(0, written, "Wrote some data but shoudn't!");
                    break
                }
                let (result, read, written) = decoder.decode_to_utf8_without_replacement(
                    buffer,
                    &mut output_buf,
                    false
                );

                if read == 0 {
                    // Something bad happened.
                    panic!("Was not able to convert a single byte of {}, something went horribly wrong! Deactivate the big-file feature or contact the developer!", page.url);
                }

                match result {
                    DecoderResult::InputEmpty => {
                        panic!("The decoding of big file failed somehow!")
                    }
                    DecoderResult::Malformed(..) => {
                        had_error = true;
                    }
                    DecoderResult::OutputFull => {}
                }


                let real_written = output.write(&output_buf[..written])?;
                assert_eq!(written, real_written, "Expected to write {written} but only wrote {real_written}!");
                reader.consume(read);
            }
            Ok(DecodedData::new_off_memory(
                out_path,
                encoding,
                had_error,
            ))
        },
        DataHolder::None => unreachable!()
    }
}



#[cfg(test)]
mod test {
    use std::borrow::Cow;
    use encoding_rs::{Encoding};
    use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
    use reqwest::StatusCode;
    use crate::core::decoding::{decode};
    use crate::core::{UrlWithDepth, DataHolder};
    use crate::core::response::{ResponseData};
    use crate::core::fetching::{FetchedRequestData};
    use crate::core::contexts::inmemory::InMemoryContext;
    use crate::core::format::AtraFileInformation;

    macro_rules! test_for {
        (@old $name: ident: $sample: ident($encoding: expr)) => {
            #[allow(non_snake_case)]
            #[tokio::test]
            async fn $name(){
                let original_enc = $encoding;
                let (website, content) = $sample(original_enc);
                let context = InMemoryContext::default();
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
                let context = InMemoryContext::default();
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


    fn encode<'a>(encoding: &'static Encoding, data: &'a str) -> (Cow<'a, [u8]>, &'static Encoding, bool) {
        encoding.encode(data)
    }

    fn website_old(encoding: &'static Encoding) -> (ResponseData, &'static str) {
        const DATA: &'static str = include_str!("../samples/sample_1.html");
        let (content, used_enc, _) = encode(encoding, DATA);
        assert_eq!(encoding, used_enc, "The used encoding {} differs from the expected one {}", used_enc.name(), encoding.name());
        let data = FetchedRequestData::new(
            DataHolder::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false
        );
        (
            ResponseData::new(data, UrlWithDepth::from_seed("https://www.example.com").unwrap()),
            DATA
        )
    }



    fn website_modern1(encoding: &'static Encoding) -> (ResponseData, &'static str) {
        const DATA: &'static str = include_str!("../samples/sample_1.html");
        let (content, used_enc, _) = encode(encoding, DATA);
        assert_eq!(encoding, used_enc, "The used encoding {} differs from the expected one {}", used_enc.name(), encoding.name());
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::try_from(format!("text/html; charset={}", encoding.name())).unwrap());
        let data = FetchedRequestData::new(
            DataHolder::from_vec(content.to_vec()),
            Some(headers),
            StatusCode::OK,
            None,
            None,
            false
        );
        (
            ResponseData::new(data, UrlWithDepth::from_seed("https://www.example.com").unwrap()),
            DATA
        )
    }



    fn website_modern2(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../samples/sample_2.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(encoding, used_enc, "The used encoding {} differs from the expected one {}", used_enc.name(), encoding.name());
        let data = FetchedRequestData::new(
            DataHolder::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false
        );
        (
            ResponseData::new(data, UrlWithDepth::from_seed("https://www.example.com").unwrap()),
            replaces
        )
    }

    fn website_modern3(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../samples/sample_3.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(encoding, used_enc, "The used encoding {} differs from the expected one {}", used_enc.name(), encoding.name());
        let data = FetchedRequestData::new(
            DataHolder::from_vec(content.to_vec()),
            None,
            StatusCode::OK,
            None,
            None,
            false
        );
        (
            ResponseData::new(data, UrlWithDepth::from_seed("https://www.example.com").unwrap()),
            replaces
        )
    }

    fn website_modern4(encoding: &'static Encoding) -> (ResponseData, String) {
        const DATA: &'static str = include_str!("../samples/sample_4.html");
        let replaces = DATA.replace("UTF-8", encoding.name());
        let (content, used_enc, _) = encode(encoding, &replaces);
        assert_eq!(encoding, used_enc, "The used encoding {} differs from the expected one {}", used_enc.name(), encoding.name());
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::try_from(format!("text/html; charset={}", encoding.name())).unwrap());
        let data = FetchedRequestData::new(
            DataHolder::from_vec(content.to_vec()),
            Some(headers),
            StatusCode::OK,
            None,
            None,
            false
        );
        (
            ResponseData::new(data, UrlWithDepth::from_seed("https://www.example.com").unwrap()),
            replaces
        )
    }

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