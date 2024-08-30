use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use file_format::FileFormat;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use crate::core::contexts::Context;
use crate::core::format::mime::MimeType;
use crate::core::response::ResponseData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetectedFileFormat {
    Unambiguous(FileFormat),
    Ambiguous(FileFormat, i32, SmallVec<[(FileFormat, i32); 1]>),
}

impl DetectedFileFormat {
    pub fn most_probable_file_format(&self) -> &FileFormat {
        match self {
            DetectedFileFormat::Unambiguous(value) | DetectedFileFormat::Ambiguous(value, _, _) => value,
        }
    }
}


/// Infers the file format for some kind of data.
pub(crate) fn infer_file_formats(
    page: &ResponseData,
    mime: Option<&MimeType>,
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

    if let Some(mime) = mime {
        for mim in mime.iter() {
            if let Some(infered) =  FileFormat::from_media_type(mim.essence_str()) {
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
            let mut result: VecDeque<_> = formats.into_iter().sorted_by(|(_, cta), (_, ctb)| ctb.cmp(cta)).collect();
            let (most_probable, ct) = result.pop_front().unwrap();
            Some(DetectedFileFormat::Ambiguous(most_probable, ct, SmallVec::from_slice(result.make_contiguous())))
        }
    }
}