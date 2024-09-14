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

use crate::client::traits::{AtraClient, AtraResponse};
use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::RawData;
use crate::fetching::FetchedRequestData;
use crate::io::fs::AtraFS;
use bytes::Bytes;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{IntoUrl, StatusCode};
use reqwest_middleware::ClientWithMiddleware;
use std::io::{Read, Seek, Write};
use std::num::IntErrorKind;
use tempfile::NamedTempFile;
use tokio_stream::StreamExt;
use ubyte::ToByteUnit;

impl AtraResponse for reqwest::Response {
    type Error = reqwest_middleware::Error;
    type Bytes = Bytes;

    #[inline(always)]
    fn status(&self) -> StatusCode {
        reqwest::Response::status(self)
    }

    #[inline(always)]
    async fn text(self) -> Result<String, Self::Error> {
        Ok(reqwest::Response::text(self).await?)
    }

    #[inline(always)]
    async fn bytes(self) -> Result<Self::Bytes, Self::Error> {
        Ok(reqwest::Response::bytes(self).await?)
    }
}

pub struct ClientWithUserAgent {
    user_agent: String,
    inner: ClientWithMiddleware,
}

impl ClientWithUserAgent {
    pub fn new(user_agent: String, inner: ClientWithMiddleware) -> Self {
        Self { user_agent, inner }
    }
}

impl AtraClient for ClientWithUserAgent {
    type Error = reqwest_middleware::Error;
    type Response = reqwest::Response;

    fn user_agent(&self) -> &str {
        &self.user_agent
    }

    async fn get<U>(&self, url: U) -> Result<Self::Response, Self::Error>
    where
        U: IntoUrl,
    {
        self.inner.get(url).send().await
    }

    async fn retrieve<C, U>(&self, context: &C, url: U) -> Result<FetchedRequestData, Self::Error>
    where
        C: SupportsConfigs + SupportsFileSystemAccess,
        U: IntoUrl,
    {
        let target_url_str = url.as_str();
        match self.inner.get(url.as_str()).send().await {
            Ok(res) => {
                let u = res.url().as_str();
                let rd = if target_url_str != u {
                    Some(u.into())
                } else {
                    None
                };

                let headers = res.headers();
                let mut can_download = true;
                let mut can_download_in_memory = false;

                let content_length_in_bytes = match res.content_length() {
                    None => {
                        if let Some(size_hint) = headers.get(CONTENT_LENGTH) {
                            if let Ok(length) = size_hint.to_str() {
                                match length.parse::<u64>() {
                                    Ok(length) => Some(length),
                                    Err(err) => {
                                        match err.kind() {
                                            IntErrorKind::Empty => {
                                                log::warn!(
                                                    "{}: The content-length of is empty.",
                                                    url.as_str()
                                                )
                                            }
                                            IntErrorKind::InvalidDigit => {
                                                log::warn!("{}: The content-length has invalid digits: {length}", url.as_str())
                                            }
                                            IntErrorKind::PosOverflow => {
                                                can_download = false;
                                                log::warn!("{}: The content-length indicates a size greater than {}. Atra can not handle this.", target_url_str, u64::MAX.pebibytes())
                                            }
                                            IntErrorKind::NegOverflow => {
                                                log::warn!("{}: The content-length indicates a size of {length}, which is smaller than 0 bytes.", url.as_str())
                                            }
                                            IntErrorKind::Zero => unreachable!(),
                                            _ => {}
                                        }
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    found => found,
                };

                if let Some(found) = content_length_in_bytes {
                    if let Some(max_size) = context.configs().crawl.max_file_size {
                        can_download = found <= max_size.get();
                    }
                    can_download_in_memory =
                        found <= context.configs().system.max_file_size_in_memory;
                } else {
                    // todo: make something better???
                    match headers.get(CONTENT_TYPE) {
                        None => {}
                        Some(value) => match value.to_str() {
                            Ok(value) => {
                                can_download_in_memory = value.to_lowercase().contains("text/html");
                            }
                            Err(_) => {}
                        },
                    }
                }

                let headers = Some(headers.clone());
                let status_code = res.status();
                let address = res.remote_addr();

                fn persist_temp<T>(
                    temp: NamedTempFile,
                    context: &impl SupportsFileSystemAccess,
                    target_url_str: &str,
                ) -> std::result::Result<RawData<T>, RawData<T>> {
                    let path = context.fs().create_unique_path_for_dat_file(target_url_str);
                    match temp.persist(&path) {
                        Ok(_) => Ok(RawData::from_external(path)),
                        Err(err) => {
                            log::error!("{target_url_str}: Had problems persisting the downloaded data as file: {err}");
                            Err(RawData::from_external(path))
                        }
                    }
                }

                let mut defect = false;

                let content = if can_download {
                    if can_download_in_memory {
                        if let Some(value) = res.bytes().await.ok().map(|value| value.to_vec()) {
                            RawData::from_vec(value)
                        } else {
                            RawData::None
                        }
                    } else {
                        match NamedTempFile::new() {
                            Ok(mut temp) => {
                                let mut stream = res.bytes_stream();

                                let mut bytes_downloaded = 0u64;

                                while let Some(chunk) = stream.next().await {
                                    match chunk {
                                        Ok(result) => {
                                            bytes_downloaded += result.len() as u64;
                                            match temp.write_all(&result) {
                                                Err(err) => {
                                                    defect = true;
                                                    log::error!("{target_url_str}: Had an error while writing to tempfile {temp:?}! {err}");
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                        Err(err) => {
                                            defect = true;
                                            log::error!("{target_url_str}: Had an error while downloading the stream to tempfile {temp:?}! {err}");
                                            break;
                                        }
                                    }
                                }

                                if let Ok(meta) = temp.as_file().metadata() {
                                    if meta.len() != bytes_downloaded {
                                        defect = true;
                                        log::warn!("{target_url_str}: Number of bytes downloaded {bytes_downloaded} differs from bytes written to tempfile {}", meta.len());
                                    }
                                    if meta.len()
                                        <= context.configs().system.max_file_size_in_memory
                                    {
                                        match temp.rewind() {
                                            Ok(_) => {
                                                let mut buf =
                                                    Vec::with_capacity(meta.len() as usize);
                                                match temp.read_to_end(&mut buf) {
                                                    Ok(read) => {
                                                        if read != meta.len() as usize {
                                                            log::info!("{target_url_str}: The size of the tempfile {} differs from the read size {}", meta.len(), read);
                                                        }
                                                        if buf.is_empty() {
                                                            RawData::None
                                                        } else {
                                                            RawData::from_vec(buf)
                                                        }
                                                    }
                                                    Err(err) => {
                                                        defect = true;
                                                        log::warn!("{target_url_str}: Had an error while reading the temp file {temp:?}: {err}");
                                                        let path = context
                                                            .fs()
                                                            .create_unique_path_for_dat_file(
                                                                target_url_str,
                                                            );
                                                        match temp.persist(&path) {
                                                            Ok(_) => {}
                                                            Err(err) => {
                                                                log::error!("{target_url_str}: Had problems persisting the downloaded data as file: {err}")
                                                            }
                                                        }
                                                        RawData::from_external(path)
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                log::error!(
                                                    "Failed to work with temp file {:?}: {err}",
                                                    temp
                                                );
                                                RawData::None
                                            }
                                        }
                                    } else {
                                        match persist_temp(temp, context, url.as_str()) {
                                            Ok(result) => result,
                                            Err(result) => {
                                                defect = true;
                                                result
                                            }
                                        }
                                    }
                                } else {
                                    match persist_temp(temp, context, url.as_str()) {
                                        Ok(result) => result,
                                        Err(result) => {
                                            defect = true;
                                            result
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                defect = true;
                                log::error!("{target_url_str}: Was not able to download the file due to error when creating a temp file: {err}");
                                RawData::None
                            }
                        }
                    }
                } else {
                    RawData::None
                };

                Ok(FetchedRequestData {
                    headers,
                    final_url: rd,
                    status_code,
                    address,
                    content,
                    defect,
                })
            }
            Err(error) => {
                log::debug!("error fetching {} - {}", target_url_str, error);
                Err(error)
            }
        }
    }
}
