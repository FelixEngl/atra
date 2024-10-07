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

use std::fs::File;
use std::io;
use std::io::{BufReader, Read};
use actix_web::{HttpResponse, Responder, ResponseError, web};
use actix_web::error::JsonPayloadError;
use actix_web::http::StatusCode;
use itertools::{Either, Itertools};
use rocksdb::IteratorMode;
use thiserror::Error;
use tokio::io::BufStream;
use ubyte::{ByteUnit, ToByteUnit};
use crate::contexts::local::LocalContext;
use crate::crawl::SlimCrawlResult;
use crate::data::RawVecData;
use crate::database::DatabaseError;
use crate::url::AtraUri;
use crate::warc_ext::ReaderError;

pub async fn get_list(context: web::Data<LocalContext>) -> impl Responder {
    context.get_ref().crawl_db().iter(IteratorMode::Start)
}

#[derive(Debug, Error)]
enum AtraResponseError {
    #[error(transparent)]
    JsonPayload(#[from] JsonPayloadError),
    #[error(transparent)]
    DatabaseError(#[from] DatabaseError),
    #[error(transparent)]
    ReaderError(#[from] ReaderError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("No meta was found for the request.")]
    NoMetaFound,
    #[error("No data was found for the request.")]
    NoDataFound,
}

impl ResponseError for AtraResponseError {
    fn status_code(&self) -> StatusCode {
        match self {
            AtraResponseError::JsonPayload(_) => {StatusCode::BAD_REQUEST}
            AtraResponseError::DatabaseError(_) => {StatusCode::INTERNAL_SERVER_ERROR}
            AtraResponseError::NoMetaFound => {StatusCode::NOT_FOUND}
            AtraResponseError::NoDataFound => {StatusCode::NOT_FOUND}
            AtraResponseError::ReaderError(_) => {StatusCode::INTERNAL_SERVER_ERROR}
            AtraResponseError::Io(_) => {StatusCode::INTERNAL_SERVER_ERROR}
        }
    }
}

pub async fn get_meta(context: web::Data<LocalContext>, target: web::JsonBody<AtraUri>) -> Result<SlimCrawlResult, AtraResponseError> {
    match context.get_ref().crawl_db().get(&target.await?)? {
        None => {
            Err(AtraResponseError::NoMetaFound)
        }
        Some(value) => {
            Ok(value)
        }
    }
}

pub async fn get_data(context: web::Data<LocalContext>, target: web::JsonBody<AtraUri>) -> Result<HttpResponse, AtraResponseError> {
    match context.get_ref().crawl_db().get(&target.await?)? {
        None => {
            Err(AtraResponseError::NoMetaFound)
        }
        Some(value) => {
            match unsafe{value.get_content()?} {
                Either::Left(data) => {
                    match data {
                        RawVecData::None => {
                            Err(AtraResponseError::NoDataFound)
                        }
                        RawVecData::InMemory { data, .. } => {
                            Ok(
                                HttpResponse::Ok()
                                    .content_type(actix_web::http::header::ContentType(value.meta.file_information.get_best_mime_type().into_owned()))
                                    .body(data)
                            )
                        }
                        RawVecData::ExternalFile { path, .. } => {
                            Ok(
                                HttpResponse::Ok()
                                    .content_type(actix_web::http::header::ContentType(value.meta.file_information.get_best_mime_type().into_owned()))
                                    .streaming(
                                        tokio_stream::iter(
                                            BufReader::new(File::options().write(true).read(true).open(&path)?).bytes()
                                        )
                                    )
                            )
                        }
                    }
                }
                Either::Right(bytes) => {
                    Ok(
                        HttpResponse::Ok()
                            .content_type(actix_web::http::header::ContentType(value.meta.file_information.get_best_mime_type().into_owned()))
                            .body(bytes)
                    )
                }
            }
        }
    }
}