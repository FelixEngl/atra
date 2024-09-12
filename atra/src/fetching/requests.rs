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

use crate::client::Client;
use crate::client::Result;
use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::{RawData, RawVecData};
use crate::io::fs::AtraFS;
use log;
use reqwest::header::{HeaderMap, CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{IntoUrl, StatusCode};
use std::io::{Read, Seek, Write};
use std::net::SocketAddr;
use std::num::IntErrorKind;
use tempfile::NamedTempFile;
use tokio_stream::StreamExt;
use ubyte::ToByteUnit;

/// The response of a fetch.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct FetchedRequestData {
    /// A dataholder with the body of a fetched request.
    pub content: RawVecData,
    /// The headers of the response. (Always None if a webdriver protocol is used for fetching.).
    pub headers: Option<HeaderMap>,
    /// The status code of the request.
    pub status_code: StatusCode,
    /// The final url destination after any redirects.
    pub final_url: Option<String>,
    /// The remote address
    pub address: Option<SocketAddr>,
    /// Set if there was an error
    pub defect: bool,
}

impl FetchedRequestData {
    #[cfg(test)]
    pub fn new(
        content: RawVecData,
        headers: Option<HeaderMap>,
        status_code: StatusCode,
        final_url: Option<String>,
        address: Option<SocketAddr>,
        defect: bool,
    ) -> Self {
        Self {
            content,
            headers,
            status_code,
            final_url,
            address,
            defect,
        }
    }
}

/// Perform a network request to a resource extracting all content
pub async fn fetch_request<C, U: IntoUrl>(
    context: &C,
    client: &Client,
    target_url: U,
) -> Result<FetchedRequestData>
where
    C: SupportsConfigs + SupportsFileSystemAccess,
    U: IntoUrl,
{
    let target_url_str = target_url.as_str();
    match client.get(target_url_str).send().await {
        Ok(res) => {
            let u = res.url().as_str();
            let rd = if target_url.as_str() != u {
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
                                                "{target_url_str}: The content-length of is empty."
                                            )
                                        }
                                        IntErrorKind::InvalidDigit => {
                                            log::warn!("{target_url_str}: The content-length has invalid digits: {length}")
                                        }
                                        IntErrorKind::PosOverflow => {
                                            can_download = false;
                                            log::warn!("{target_url_str}: The content-length indicates a size greater than {}. Atra can not handle this.", u64::MAX.pebibytes())
                                        }
                                        IntErrorKind::NegOverflow => {
                                            log::warn!("{target_url_str}: The content-length indicates a size of {length}, which is smaller than 0 bytes.")
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
                if let Some(max_size) = context.configs().crawl().max_file_size {
                    can_download = found <= max_size.get();
                }
                can_download_in_memory =
                    found <= context.configs().system().max_file_size_in_memory;
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
                                if meta.len() <= context.configs().system().max_file_size_in_memory
                                {
                                    match temp.rewind() {
                                        Ok(_) => {
                                            let mut buf = Vec::with_capacity(meta.len() as usize);
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
                                    let path = context
                                        .fs()
                                        .create_unique_path_for_dat_file(target_url_str);
                                    match temp.persist(&path) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            defect = true;
                                            log::error!("{target_url_str}: Had problems persisting the downloaded data as file: {err}")
                                        }
                                    }
                                    RawData::from_external(path)
                                }
                            } else {
                                let path =
                                    context.fs().create_unique_path_for_dat_file(target_url_str);
                                match temp.persist(&path) {
                                    Ok(_) => {}
                                    Err(err) => {
                                        defect = true;
                                        log::error!("{target_url_str}: Had problems persisting the downloaded data as file: {err}")
                                    }
                                }
                                RawData::from_external(path)
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
            log::debug!("error fetching {} - {}", target_url.as_str(), error);
            Err(error)
        }
    }
}
