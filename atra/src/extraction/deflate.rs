// Copyright 2024. Felix Engl
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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess, SupportsGdbrRegistry};
use crate::data::{Decoded, RawVecData};
use crate::decoding::{decode};
use crate::extraction::extractor::{ExtractorData, ExtractorResult};
use crate::extraction::{LinkExtractionError};
use crate::format::{determine_format, FileContentReader, FileFormatData, ZipFileContent};
use crate::io::serial::{SerialProvider, SerialProviderKind};
use crate::io::templating::{file_name_template, FileNameTemplate};
use crate::io::unique_path_provider::{UniquePathProvider, UniquePathProviderWithTemplate};
use crate::toolkit::{detect_language};
use crate::url::UrlWithDepth;
use camino_tempfile::Utf8TempDir;
use std::io::{Read, Seek};
use std::sync::{LazyLock};
use tokio::task::yield_now;

/// Extract data fom
pub async fn extract_from_zip<C, R>(
    root_url: &UrlWithDepth,
    reader: R,
    nesting: usize,
    context: &C,
) -> Result<(
    Vec<(String, ExtractorResult)>,
    Vec<(String, LinkExtractionError)>,
), LinkExtractionError>
where
    C: SupportsGdbrRegistry + SupportsConfigs + SupportsFileSystemAccess,
    R: Read + Seek,
{
    let mut archive = zip::read::ZipArchive::new(reader)?;
    let mut extracted = Vec::new();
    let mut errors = Vec::with_capacity(0);
    // let extracted_result = HashSet::new();
    let len = archive.len();
    let temp_dir = Utf8TempDir::new()?;

    static TEMPLATE: LazyLock<FileNameTemplate> = LazyLock::new(
        || file_name_template!("unpacked" _ serial ".tmp").unwrap()
    );

    let name_provider = UniquePathProviderWithTemplate::new(
        UniquePathProvider::new(
            temp_dir.path(),
            SerialProvider::new(SerialProviderKind::Long),
        ),
        TEMPLATE.clone(),
    );

    for idx in 0..len {
        yield_now().await;
        let mut content = ZipFileContent::new(
            &mut archive,
            idx,
            context.configs().system.max_file_size_in_memory as usize,
            None,
        );

        let (file_name, data, file_info) = match content.file_name_and_len() {
            Ok(Some((file_name, len))) => {
                if len == 0 {
                    log::debug!("Only found empty file!");
                    continue;
                }

                let determined = determine_format(
                    context,
                    FileFormatData::new(None, &mut content, None, Some(&file_name)),
                );

                if !context
                    .configs()
                    .crawl
                    .link_extractors
                    .can_extract_anything(&determined)
                {
                    log::debug!("Can not extract from {file_name}");
                    continue;
                }

                let cfg_sys = &context.configs().system;
                let data = if len <= cfg_sys.max_temp_file_size_on_disc {
                    let temp_file_name = match name_provider.provide_path_no_args() {
                        Ok(value) => {
                            value
                        }
                        Err(err) => {
                            log::error!("Failed to use template for temp files: {err}");
                            continue;
                        }
                    };
                    match content.zip_reader().extract(&temp_file_name) {
                        Ok(_) => {}
                        Err(err) => {
                            errors.push((file_name, err.into()));
                            continue;
                        }
                    }
                    RawVecData::from_external(temp_file_name)
                } else {
                    match content.cursor() {
                        Ok(Some(mut value)) => {
                            if len <= cfg_sys.max_file_size_in_memory {
                                let mut data = Vec::with_capacity(len as usize);
                                match value.read_to_end(&mut data) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        errors.push((file_name, err.into()));
                                        continue;
                                    }
                                }
                                RawVecData::from_in_memory(data)
                            } else {
                                log::warn!("A file was too big for extracting data.");
                                continue;
                            }
                        }
                        Ok(None) => continue,
                        Err(err) => {
                            errors.push((file_name, err.into()));
                            continue;
                        }
                    }
                };

                (file_name, data, determined)
            }
            Ok(None) => {
                /*Do nothing, it is a dir or whatever*/
                continue;
            }
            Err(error) => {
                errors.push((root_url.as_str().into_owned(), error.into()));
                continue;
            }
        };

        log::debug!("Read {file_name} for {}", root_url.url);

        let result = match decode(context, &data, &file_name, None, &file_info)
            .await
            .map(|value| value.map_in_memory(|value| value.into_owned()))
        {
            Ok(decoded) => {
                let lang = detect_language(context, &file_info, &decoded)
                    .ok()
                    .flatten();

                context
                    .configs()
                    .crawl
                    .link_extractors
                    .extract(
                        context,
                        nesting + 1,
                        ExtractorData::new(
                            root_url,
                            Some(&file_name),
                            &data,
                            &file_info,
                            &decoded,
                            lang.as_ref(),
                        ),
                    ).await
            }
            Err(_) => {
                // If we have an encoding error at this stage we simply do not care.
                context
                    .configs()
                    .crawl
                    .link_extractors
                    .extract(
                        context,
                        nesting + 1,
                        ExtractorData::new(
                            root_url,
                            Some(&file_name),
                            &data,
                            &file_info,
                            &Decoded::None,
                            None,
                        ),
                    ).await
            }
        };

        extracted.push((file_name, result))
    }

    Ok((extracted, errors))
}


#[cfg(test)]
mod test {
    use log4rs::append::console::ConsoleAppender;
    use log4rs::config::{Appender, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;
    use log::LevelFilter;
    use crate::config::Config;
    use crate::data::RawVecData;
    use crate::extraction::deflate::extract_from_zip;
    use crate::format::{determine_format, FileFormatData};
    use crate::format::supported::InterpretedProcessibleFileFormat;
    use crate::test_impls::{DefaultAtraProvider, TestContext};
    use crate::url::UrlWithDepth;

    #[tokio::test]
    async fn can_extract_from_jar_file(){
        let console_logger = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l}{I} - {d} - {m}{n}")))
            .build();

        let config = log4rs::Config::builder()
            .appender(Appender::builder().build("out", Box::new(console_logger)))
            .logger(Logger::builder().build("atra", LevelFilter::Debug))
            .build(Root::builder().appender("out").build(LevelFilter::Warn))
            .unwrap();

        let _ = log4rs::init_config(config).unwrap();

        const JAR_FILE_1: &[u8] = include_bytes!("../../testdata/samples/expressionless-0.1.0.jar");

        let cont = TestContext::new(
            Config::default(),
            DefaultAtraProvider::default()
        );

        let mut dat = RawVecData::from_in_memory(
            Vec::from(JAR_FILE_1)
        );

        let format = determine_format(
            &cont,
            FileFormatData::new(
                None,
                &mut dat,
                Some(&UrlWithDepth::from_url("https://www.google.de/expressionless.jar").unwrap()),
                None
            )
        );

        assert_eq!(format.format, InterpretedProcessibleFileFormat::ZIP);


        let (result, err) = extract_from_zip(
            &"https://www.google.de/expressionless.jar".parse().unwrap(),
            dat.cursor().expect("There was an error when getting data").expect("There is nothing to read."),
            0,
            &cont
        ).await.unwrap();

        for value in result {
            println!("{value:?}")
        }
        println!("Errors:");
        for value in err {
            println!("{value:?}")
        }

    }
}