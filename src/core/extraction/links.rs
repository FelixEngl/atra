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

use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};
use crate::core::extraction::marker::ExtractorMethodHint;
use crate::core::url::atra_uri::ParseError;
use crate::core::UrlWithDepth;

/// An extracted link, this is either a new URL or the base URL with some
#[derive(Debug, Eq, Serialize, Deserialize, Clone)]
pub enum ExtractedLink  {
    OnSeed {
        url: UrlWithDepth,
        extraction_method: ExtractorMethodHint
    },
    Outgoing {
        url: UrlWithDepth,
        extraction_method: ExtractorMethodHint
    },
    Data {
        /// Base of the url
        base: UrlWithDepth,
        /// Data url
        url: UrlWithDepth,
        /// Extraction method
        extraction_method: ExtractorMethodHint
    }
}


impl Display for ExtractedLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractedLink::OnSeed{url,..} => {
                write!(f, "OnSite::{}", url)
            }
            ExtractedLink::Outgoing{url,..} => {
                write!(f, "Outgoing::{}", url)
            }
            ExtractedLink::Data{base, url, .. } => {
                write!(f, "Data::{} - {}", base, url)
            }
        }
    }
}

impl ExtractedLink {
    // pub fn url(&self) -> &UrlWithDepth {
    //     match self {
    //         ExtractedLink::OnSeed { url, .. } => {url}
    //         ExtractedLink::Outgoing { url, .. } => {url}
    //         ExtractedLink::Data { url, .. } => {url}
    //     }
    // }

    pub fn is_not(&self, url: &UrlWithDepth) -> bool {
        match self {
            ExtractedLink::OnSeed{url:known, ..} => {url != known}
            ExtractedLink::Outgoing{url:known, ..} => {url != known}
            _ => false
        }
    }
}

impl Hash for ExtractedLink {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ExtractedLink::Outgoing{url, ..} => {
                url.hash(state)
            }
            ExtractedLink::OnSeed{url, ..} => {
                url.hash(state)
            }
            ExtractedLink::Data{base, url, ..} => {
                base.hash(state);
                url.hash(state)
            }
        }
    }
}

impl PartialEq<Self> for ExtractedLink {
    fn eq(&self, other: &Self) -> bool {
        match self {
            ExtractedLink::Outgoing{url, ..} => {
                match other {
                    ExtractedLink::Outgoing{url: o_url, ..} => {
                        url == o_url
                    }
                    _ => false
                }
            }
            ExtractedLink::OnSeed{url, ..} => {
                match other {
                    ExtractedLink::OnSeed{url: o_url, ..} => {
                        url == o_url
                    }
                    _ => false
                }
            }
            ExtractedLink::Data{base, url, .. } => {
                match other {
                    ExtractedLink::Data{base: o_base, url: o_url, ..} => {
                        o_base == base && o_url == url
                    }
                    _ => false
                }
            }
        }
    }
}

impl ExtractedLink {
    /// Packs the extracted [url] and applies [base] if necessary.
    pub fn pack(base:  &UrlWithDepth, url: &str, extraction_method: ExtractorMethodHint) -> Result<Self, ParseError> {
        if url.starts_with("data:") {
            let url = UrlWithDepth::new_like_with_base(base, url)?;
            Ok(ExtractedLink::Data {
                base: base.clone(),
                url,
                extraction_method
            })
        } else {
            let next = UrlWithDepth::with_base(base, url)?;
            if base.depth().distance_to_seed != next.depth().distance_to_seed {
                Ok(Self::Outgoing {
                    url: next,
                    extraction_method
                })
            } else {
                Ok(Self::OnSeed {
                    url: next,
                    extraction_method
                })
            }
        }
    }
}

