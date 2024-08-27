use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use file_format::FileFormat;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use crate::core::contexts::Context;
use crate::core::format::mime::MimeDescriptor;
use crate::core::response::ResponseData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetectedFileFormat {
    Unambiguous(FileFormat),
    Ambiguous(FileFormat, SmallVec<[FileFormat; 1]>),
}


/// Infers the file format for some kind of data.
pub(crate) fn infer_file_formats(
    page: &ResponseData,
    mime: &MimeDescriptor,
    context: &impl Context
) -> Option<DetectedFileFormat> {

    let mut formats = HashMap::new();
    if let Ok(Some(value)) = page.content.cursor(context) {
        match FileFormat::from_reader(value) {
            Ok(value) => {
                formats.insert(value, 1);
            }
            Err(err) => {
                log::warn!("Failed to analyze with {err}");
            }
        }
    }

    for mim in mime.iter() {
        if let Some(infered) =  FileFormat::from_media_type(mim.1.essence_str()) {
            for inf in infered {
                match formats.entry(*inf) {
                    Entry::Occupied(mut value) => {
                        *value.get_mut() += 1;
                    }
                    Entry::Vacant(value) => {
                        value.insert(1);
                    }
                }
            }
        }
    }

    if let Some(file_extension) = page.url.url().file_extension() {
        if let Some(infered) = FileFormat::from_extension(file_extension) {
            for inf in infered {
                match formats.entry(*inf) {
                    Entry::Occupied(mut value) => {
                        *value.get_mut() += 1;
                    }
                    Entry::Vacant(value) => {
                        value.insert(1);
                    }
                }
            }
        }
    }


    match formats.len() {
        0 => {
            None
        }
        1 => {
            Some(DetectedFileFormat::Unambiguous(formats.into_keys().exactly_one().unwrap()))
        }
        _ => {
            let mut result: VecDeque<_> = formats.into_iter().sorted_by(|(_, cta), (_, ctb)| ctb.cmp(cta)).map(|(value, _)| value).collect();
            let most_probable = result.pop_front().unwrap();
            Some(DetectedFileFormat::Ambiguous(most_probable, SmallVec::from_slice(result.make_contiguous())))
        }
    }
}