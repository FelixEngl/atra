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

mod errors;
mod instructions;
mod read;
mod skip_pointer;
mod special_writer;
mod write;

pub use errors::*;
pub use instructions::WarcSkipInstruction;
pub use read::read_body;
pub use skip_pointer::*;
pub use special_writer::SpecialWarcWriter;
pub use write::write_warc;

#[cfg(test)]
mod test {
    use crate::crawl::CrawlResult;
    use crate::data::RawVecData;
    use crate::fetching::FetchedRequestData;
    use crate::fetching::ResponseData;
    use crate::format::mime::MimeType;
    use crate::format::supported::InterpretedProcessibleFileFormat;
    use crate::format::AtraFileInformation;
    use crate::toolkit::LanguageInformation;
    use crate::url::UrlWithDepth;
    use crate::warc_ext::special_writer::MockSpecialWarcWriter;
    use crate::warc_ext::write_warc;
    use camino::Utf8PathBuf;
    use encoding_rs;
    use reqwest::StatusCode;
    use time::OffsetDateTime;

    #[test]
    fn can_write_html() {
        const HTML_DATA: &str = "<html><body>Hello World!</body></html>";
        let result = CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(
                FetchedRequestData::new(
                    RawVecData::from_vec(HTML_DATA.as_bytes().to_vec()),
                    None,
                    StatusCode::OK,
                    None,
                    None,
                    false,
                ),
                UrlWithDepth::from_seed("https://www.google.de/0").unwrap(),
            ),
            None,
            Some(encoding_rs::UTF_8),
            AtraFileInformation::new(
                InterpretedProcessibleFileFormat::HTML,
                Some(MimeType::new_single(mime::TEXT_HTML_UTF_8)),
                None,
            ),
            Some(LanguageInformation::ENG),
        );

        let mut special = MockSpecialWarcWriter::new();

        special
            .expect_get_skip_pointer()
            .returning(|| Ok((Utf8PathBuf::new(), 0)));

        special.expect_write_header().return_once(|value| {
            let value = value.to_string();
            println!("Header:\n{value}");
            Ok(value.len())
        });

        special.expect_write_body_complete().return_once(|value| {
            println!("Body:\n{}", String::from_utf8_lossy(value));
            Ok(value.len())
        });

        special.expect_forward_if_filesize().returning(|_| Ok(None));

        let instruction = write_warc(&mut special, &result).expect("Should work!");

        println!("{instruction:?}")
    }

    #[test]
    fn can_write_base64() {
        const HTML_DATA: &str = "<html><body>Hello World! WARBLGARBL</body></html>";
        let result = CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(
                FetchedRequestData::new(
                    RawVecData::from_vec(HTML_DATA.as_bytes().to_vec()),
                    None,
                    StatusCode::OK,
                    None,
                    None,
                    false,
                ),
                UrlWithDepth::from_seed("https://www.google.de/0").unwrap(),
            ),
            None,
            Some(encoding_rs::UTF_8),
            AtraFileInformation::new(InterpretedProcessibleFileFormat::Unknown, None, None),
            Some(LanguageInformation::ENG),
        );

        let mut special = MockSpecialWarcWriter::new();

        special
            .expect_get_skip_pointer()
            .returning(|| Ok((Utf8PathBuf::new(), 0)));

        special.expect_write_header().return_once(|value| {
            let value = value.to_string();
            println!("Header:\n{value}");
            Ok(value.len())
        });

        special.expect_write_body_complete().return_once(|value| {
            println!("Body:\n{}", String::from_utf8_lossy(value));
            Ok(value.len())
        });

        special.expect_forward_if_filesize().returning(|_| Ok(None));

        let instruction = write_warc(&mut special, &result).expect("Should work!");

        println!("{instruction:?}")
    }
}
