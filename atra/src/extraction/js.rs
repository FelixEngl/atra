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

use compact_str::{CompactString, ToCompactString};
use ress::prelude::{StringLit, Token};
use ress::tokens::Punct;
use ress::Scanner;
use std::collections::HashSet;

/// Tries to extract all links from a js-script.
pub fn extract_links(script: &str) -> HashSet<CompactString> {
    let scanner = Scanner::new(script);

    let mut links = HashSet::new();
    let mut href_found = false;

    for item in scanner {
        match item {
            Ok(ref value) => {
                match &value.token {
                    Token::Ident(identifier) => {
                        if "href" == identifier.as_ref() {
                            href_found = true;
                        }
                    }
                    Token::Punct(Punct::SemiColon) => {
                        if href_found {
                            log::trace!("JS_Extract: Missed some href at {item:?}!");
                            href_found = false
                        }
                    }
                    // TODO: handle string concat?????
                    Token::String(value) => {
                        if !href_found {
                            continue;
                        }
                        let link = match value {
                            StringLit::Single(value) => value.content.to_compact_string(),
                            StringLit::Double(value) => value.content.to_compact_string(),
                        };
                        href_found = false;
                        links.insert(link);
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }
    }

    return links;
}

#[cfg(test)]
mod test {
    use crate::extraction::js::extract_links;
    const SCRIPT: &str = r###"
        var ele = document.createElement('a');
        ele.href = 'https://a11ywatch.com';
        "###;
    #[test]
    fn test() {
        println!("{:?}", extract_links(SCRIPT))
    }
}
