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

use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::Hash;
use compact_str::{CompactString, ToCompactString};
use scraper::Html;
use serde::{Deserialize, Serialize};
use crate::core::UrlWithDepth;

/// Describes the origin of the extracted link
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinkOrigin {
    Href,
    Embedded,
    JavaScript,
    JavaScriptEmbedded,
    OnClick
}

/// Extracts links from an html
pub fn extract_links<'a>(
    root_url: &'a UrlWithDepth,
    html: &str,
    respect_nofollow: bool,
    crawl_embedded_data: bool,
    crawl_javascript: bool,
    crawl_onclick_by_heuristic: bool,
) -> Option<(Cow<'a, UrlWithDepth>, HashSet<(LinkOrigin, CompactString)>, Vec<Cow<'static, str>>)> {
    let html = Html::parse_document(html);

    if respect_nofollow {
        if html.select(&selectors::META_NO_FOLLOW).next().is_some() {
            log::debug!("Respecting no-follow metatag of {}", root_url);
            return None;
        }
    }

    let mut result = HashSet::new();
    let base = html
        .select(&selectors::BASE)
        .into_iter()
        .next()
        .map(|base| base.attr("href")
            .into_iter()
            .next()
            .map(|it|
                UrlWithDepth::with_base(&root_url, it)
            )
        )
        .flatten()
        .transpose();

    let base = match base {
        Ok(success) => {
            if let Some(success) = success {
                Cow::Owned(success)
            } else {
                Cow::Borrowed(root_url)
            }
        }
        Err(err) => {
            log::debug!("Was not able to parse the provided base url: {}", err);
            Cow::Borrowed(root_url)
        }
    };

    for element in html.select(&selectors::HREF_HOLDER) {
        if respect_nofollow {
            if let Some(rel) = element.attr("rel") {
                if rel == "nofollow" {
                    log::trace!("Respecting no-follow");
                    continue
                }
            }
        }
        if let Some(href) = element.attr("href") {
            result.insert((LinkOrigin::Href, href.to_compact_string()));
        }
    }

    if crawl_embedded_data {
        for element in html.select(&selectors::SRC_HOLDER) {
            if let Some(src) = element.attr("src") {
                result.insert((LinkOrigin::Embedded, src.to_compact_string()));
            }
        }
    }

    if crawl_javascript {
        for element in html.select(&selectors::SCRIPT_HOLDER) {
            if let Some(src) = element.attr("src") {
                result.insert((LinkOrigin::JavaScript, src.to_compact_string()));
            } else {
                for entry in crate::core::extraction::js::extract_links(element.text().collect::<String>().as_str()) {
                    result.insert((LinkOrigin::JavaScriptEmbedded, entry));
                }
            }
        }
    }


    if crawl_onclick_by_heuristic {
        for element in html.select(&selectors::ON_CLICK) {
            let found = selectors::HREF_LOCATION_MATCHER.captures(element.attr("onclick").unwrap());
            if let Some(found) = found {
                if let Some(found) = found.get(1){
                    result.insert((LinkOrigin::OnClick, found.as_str().to_compact_string()));
                }
            }
        }
    }



    Some((base, result, html.errors))
}



mod selectors {
    use once_cell::sync::Lazy;
    use regex::Regex;
    use crate::{static_selectors};

    /*
    See https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes

    <lastmod>: für aktualisierung

    - [download]: <a>, <area>
    Indicates that the hyperlink is to be used for downloading a resource.

    - [href]: <a>, <area>, <base>, <link>
    The URL of a linked resource.

    - [language]: <script>
    Defines the script language used in the element.

    - [ping]: <a>, <area>
    The ping attribute specifies a space-separated list of URLs to be notified if a user follows the hyperlink.

    - [rel]: <a>, <area>, <link>
    Specifies the relationship of the target object to the link object.
    May have nofollow

    - [src]: <audio>, <embed>, <iframe>, <img>, <input>, <script>, <source>, <track>, <video>
    The URL of the embeddable content.

    -[srcdoc]: <iframe>
    Inline HTML to embed, overriding the src attribute.
    If a browser does not support the srcdoc attribute,
    it will fall back to the URL in the src attribute.

    - [srcset]: <img>, <source>
    One or more strings separated by commas, indicating possible image sources for the user agent to use. Each string is composed of:

    A URL to an image
    Optionally, whitespace followed by one of:
    A width descriptor (a positive integer directly followed by w). The width descriptor is divided by the source size given in the sizes attribute to calculate the effective pixel density.
    A pixel density descriptor (a positive floating point number directly followed by x).
    If no descriptor is specified, the source is assigned the default descriptor of 1x.

    It is incorrect to mix width descriptors and pixel density descriptors in the same srcset attribute. Duplicate descriptors (for instance, two sources in the same srcset which are both described with 2x) are also invalid.

    If the srcset attribute uses width descriptors, the sizes attribute must also be present, or the srcset itself will be ignored.

    The user agent selects any of the available sources at its discretion. This provides them with significant leeway to tailor their selection based on things like user preferences or bandwidth conditions. See our Responsive images tutorial for an example.

    "elva-fairy-480w.jpg 480w, elva-fairy-800w.jpg 800w"

    - [onclick]: überall z.B. <div>
    "location.href='http://www.example.com';"

    https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URLs
    data:[<mediatype>][;base64],<data>

    data:,Hello%2C%20World%21
    data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==
    data:text/html,%3Ch1%3EHello%2C%20World%21%3C%2Fh1%3E
    data:text/html,%3Cscript%3Ealert%28%27hi%27%29%3B%3C%2Fscript%3E

     */

    /*
    Protocols:
    https://en.wikipedia.org/wiki/List_of_URI_schemes

    Base can have anything except data: and javascript:
     */

    pub static HREF_LOCATION_MATCHER: Lazy<Regex> = Lazy::new(||Regex::new("location\\.href='([^']*)';").unwrap());


    // Ignore [ping] of area/a
    static_selectors! {
        pub [
            BASE = "base"
            HREF_HOLDER = "a,area,link"
            SRC_HOLDER = "audio,embed,iframe,img,input,source,track,video"
            SCRIPT_HOLDER = "script"
            // TARGET_ELEMENTS = "a,area,base,link,script,audio,embed,iframe,img,input,script,source,track,video"
            ON_CLICK = "[onclick]"
            // SCRIPT = "script"

            META_NO_FOLLOW = "meta[name=\"robots\"][content=\"nofollow\"]"
        ]
    }
}
