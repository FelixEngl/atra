use core::str;
use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use itertools::Itertools;
use mime::{Mime, MimeIter, Name, Params};
use scraper::Html;
use serde::{Deserialize, Serialize};
use crate::format::mime_serialize::for_vec;
use crate::response::ResponseData;
use crate::static_selectors;

pub use mime::*;
use crate::url::atra_uri::AtraUri;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct MimeType {
    #[serde(with = "for_vec")]
    types: Vec<Mime>
}



macro_rules! create_fn {
    (
        $($name: ident => $targ: ident<$t:ty>),+
    ) => {
        $(
            pub fn $name<'a>(&'a self) -> MimesIter<'a, $t> {
                MimesIter {
                    mimes: self.types.iter(),
                    extractor: Box::new(|value| Some(value.$targ()))
                }
            }
        )+
    };
}

impl MimeType {
    pub unsafe fn new_unchecked(types: Vec<Mime>) -> Self {
        Self { types }
    }

    pub fn new_single(mime: Mime) -> Self {
        Self {
            types: vec![mime]
        }
    }

    pub fn new(types: Vec<Mime>) -> Self {
        let mut collected = types.into_iter().unique().collect_vec();
        collected.sort();
        collected.shrink_to_fit();
        unsafe{ Self::new_unchecked(collected) }
    }

    create_fn! {
        types => type_<Name>,
        subtypes => subtype<Name>,
        essence_strs => essence_str<&str>
    }

    pub fn suffixes(&self) -> MimesIter<Name> {
        MimesIter {
            mimes: self.iter(),
            extractor: Box::new(|value| value.suffix())
        }
    }

    pub fn params(&self) -> MimeParamsIter {
        MimeParamsIter::new(self.iter())
    }

    pub fn get_param_values(&self, name: Name) -> Option<Vec<Name>> {
        let found = MimeParamsIter::new_filtered(self.iter(), name).map(|value| value.1).collect_vec();
        (!found.is_empty()).then_some(found)
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Mime> {
        self.types.iter()
    }

    pub fn names_iter(&self) -> MimesNamesIter {
        MimesNamesIter::new(self.iter())
    }
}

impl From<Vec<Mime>> for MimeType {
    fn from(value: Vec<Mime>) -> Self {
        Self::new(value)
    }
}

/// Iterates over all names excluding the parameters.
/// The order is type,subtype, suffix
pub struct MimesNamesIter<'a> {
    mimes: std::slice::Iter<'a, Mime>,
    subtype: Option<Name<'a>>,
    suffix: Option<Option<Name<'a>>>,
    finished: bool
}

impl<'a> MimesNamesIter<'a> {
    fn new(mimes: std::slice::Iter<'a, Mime>) -> Self {
        Self {
            mimes,
            subtype: None,
            suffix: None,
            finished: false
        }
    }

    /// Gets the next value or returns the finished state
    fn get_next(&mut self) -> Result<Name<'a>, bool> {
        if let Some(value) = self.subtype.take() {
            Ok(value)
        } else if let Some(Some(value)) = self.suffix.take() {
            Ok(value)
        } else {
            Err(self.finished)
        }
    }

    /// Returns the type_ and caches the other two values
    fn set_next(&mut self) -> Option<Name<'a>> {
        debug_assert!(self.subtype.is_none() && self.suffix.is_none());
        if let Some(next) = self.mimes.next() {
            self.subtype = Some(next.subtype());
            self.suffix = Some(next.suffix());
            Some(next.type_())
        } else {
            self.finished = true;
            None
        }
    }
}

impl<'a> Iterator for MimesNamesIter<'a> {
    type Item = Name<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None
        }
        match self.get_next() {
            Ok(value) => {Some(value)}
            Err(false) => {self.set_next()}
            Err(true) => {None}
        }
    }
}

pub struct MimesIter<'a, T> {
    mimes: std::slice::Iter<'a, Mime>,
    extractor: Box<dyn Fn(&'a Mime) -> Option<T>>,
}

impl<'a, T> Iterator for MimesIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let found = (&self.extractor)(self.mimes.next()?);
            if found.is_some() {
                return found
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.mimes.size_hint()
    }
}

pub struct MimeParamsIter<'a, 'b> {
    mimes: std::slice::Iter<'a, Mime>,
    current: Option<Params<'a>>,
    filter: Option<Name<'b>>
}

impl<'a> MimeParamsIter<'a, 'static> {
    fn new(mut mimes: std::slice::Iter<'a, Mime>) -> Self {
        let current = mimes.next().map(|value| value.params());
        Self {
            mimes,
            current,
            filter: None
        }
    }
}

impl<'a,'b> MimeParamsIter<'a, 'b> {
    fn new_filtered(mut mimes: std::slice::Iter<'a, Mime>, filter: Name<'b>) -> Self {
        let current = mimes.next().map(|value| value.params());
        Self {
            mimes,
            current,
            filter: Some(filter)
        }
    }
}

impl<'a, 'b> Iterator for MimeParamsIter<'a, 'b> {
    type Item = (Name<'a>, Name<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current.as_mut() {
            if let Some(ref filter) = self.filter {
                while let Some(param) = current.next() {
                    if param.0.eq(filter) {
                        return Some(param);
                    }
                }
            } else {
                if let Some(param) = current.next() {
                    return Some(param);
                }
            }
        } else {
            return None;
        }
        self.current = self.mimes.next().map(|value| value.params());
        self.next()
    }
}




pub fn determine_mime_information(
    response: &ResponseData
) -> Option<MimeType> {
    static_selectors! {
        [
            META_CONTENT = "meta[http-equiv=\"Content-Type\"][content]"
        ]
    }


    /// A thorough parsing of the webpage for finding possible mime types.
    fn parse_page_raw(url: &AtraUri, content: &[u8]) -> Vec<Mime> {
        fn extract_from_html(html: &str) -> Vec<Mime> {
            Html::parse_document(html)
                .select(&META_CONTENT)
                .filter_map(
                    |value|
                    value.attr("content").map(
                        |value|
                        MimeIter::new(value).filter_map(|value| value.ok())
                    )
                )
                .flatten()
                .collect_vec()
        }
        let found_fast = extract_from_html(&String::from_utf8_lossy(content));
        if found_fast.is_empty() && !str::from_utf8(content).is_ok() {
            if let Some((encoder, _)) = Encoding::for_bom(content) {
                return extract_from_html(&encoder.decode(content).0)
            }
            let mut enc = EncodingDetector::new();
            if enc.feed(content, true) {
                let domain = url.domain();
                let domain = domain.as_ref().map(|value| psl::domain(value.as_bytes())).flatten();
                let (selected_encoding, _) =
                    if let Some(domain) = domain {
                        enc.guess_assess(Some(domain.suffix().as_bytes()), false)
                    } else {
                        enc.guess_assess(None, false)
                    };
                return extract_from_html(&selected_encoding.decode(content).0)
            }
        }
        return found_fast
    }

    let mimes_from_header = response.headers.as_ref().map(
        |value| {
            if let Some(content_type_header_value) = value.get(reqwest::header::CONTENT_TYPE) {
                if let Ok(content_type_header_value) = content_type_header_value.to_str() {
                    Some(
                        MimeIter::new(content_type_header_value)
                            .filter_map(|value| value.ok())
                            .collect_vec()
                    )
                } else {
                    None
                }
            } else {
                None
            }
        }
    ).flatten();

    if let Some(mut mimes_from_header) = mimes_from_header {
        if mimes_from_header.iter().any(|value| value.type_() == HTML) {
            if let Some(dat) = response.content.as_in_memory() {
                mimes_from_header.extend(parse_page_raw(response.get_url_parsed(), dat.as_slice()))
            } else {
                log::debug!("Unable to parse the html because of its size: {:?}!", response.content);
            }
        }
        (!mimes_from_header.is_empty()).then(|| mimes_from_header.into())
    } else {
        mimes_from_header.map(|value| value.into())
    }
}
