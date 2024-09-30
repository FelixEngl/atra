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

use crate::toolkit::utf8::{DecodedChar, RobustUtf8Reader};
use linkify::LinkKind;
use std::cmp::max;
use std::io::{Error, Read};

#[derive(Debug, Copy, Clone)]
enum Action {
    ClearPush,
    ClearSkip,
    Push,
}

type Result<T> = std::result::Result<T, Error>;

pub fn extract_possible_urls<R: Read>(
    reader: RobustUtf8Reader<R>,
) -> Result<Vec<(String, Option<LinkKind>)>> {
    let mut reader = reader.peekable();
    let mut memory: String = String::new();

    let mut link_extractor = linkify::LinkFinder::new();
    link_extractor.url_must_have_scheme(false);
    link_extractor.kinds(&[LinkKind::Url]);

    let mut links = Vec::new();
    let mut last_pos = 0;

    while find_url_start(&mut reader, &mut memory)? {
        while let Some(value) = reader.peek() {
            match value {
                Ok(DecodedChar {
                    ch: _,
                    invalid_encounters: 0,
                }) => {
                    let next = reader.next().unwrap().unwrap();
                    if matches!(determine_action(next), Action::Push) {
                        memory.push(next.ch);
                    } else {
                        break;
                    }
                }
                Ok(DecodedChar { .. }) => {
                    break;
                }
                Err(_) => return Err(reader.next().unwrap().err().unwrap()),
            }
        }

        for link in link_extractor.links(&memory) {
            links.push((
                link.as_str().to_string(),
                match link.kind() {
                    LinkKind::Url => Some(LinkKind::Url),
                    LinkKind::Email => Some(LinkKind::Email),
                    _ => None,
                },
            ));
            last_pos = max(last_pos, link.end());
        }
        memory.clear();
    }
    Ok(links)
}

const fn determine_action(c: DecodedChar) -> Action {
    if c.ch.is_ascii_whitespace() || c.ch.is_ascii_control() {
        Action::ClearSkip
    } else if !c.encountered_only_valid() {
        Action::ClearPush
    } else {
        Action::Push
    }
}

fn find_url_start<R: Iterator<Item = Result<DecodedChar>>>(
    reader: &mut R,
    buffer: &mut String,
) -> Result<bool> {
    while let Some(value) = reader.next().transpose()? {
        match determine_action(value) {
            Action::ClearPush => {
                buffer.clear();
                buffer.push(value.ch);
            }
            Action::ClearSkip => {
                buffer.clear();
            }
            Action::Push => {
                buffer.push(value.ch);
            }
        }
        if buffer.ends_with("://") || (buffer.len() >= 4 && buffer.contains('.')) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::ops::Add;
    use super::extract_possible_urls;
    use crate::toolkit::utf8::RobustUtf8Reader;
    use bytes::Buf;
    use encoding_rs::*;
    use itertools::Itertools;

    #[test]
    fn can_find_url_1() {
        const DAT: &[u8] = b"test text my friend, whats up? http://www.google.com/eq/1 omg!";
        let found = extract_possible_urls(RobustUtf8Reader::new(DAT.to_vec().reader())).unwrap();
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(
            "http://www.google.com/eq/1",
            found.0.as_str(),
            "Failed found {}",
            found.0
        );
    }

    #[test]
    fn can_find_url_2() {
        const DAT: &[u8] = b"test text my friend, whats up? https://www.google.com/eq/1omg!";
        let found = extract_possible_urls(RobustUtf8Reader::new(DAT.to_vec().reader())).unwrap();
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(
            "https://www.google.com/eq/1omg",
            found.0.as_str(),
            "Failed found {}",
            found.0
        );
    }

    #[test]
    fn can_find_url_3() {
        const DAT: &[u8] =
            b"test text my friend, whats up? (url: https://www.google.com/eq/1omg!) whaaat?";
        let found = extract_possible_urls(RobustUtf8Reader::new(DAT.to_vec().reader())).unwrap();
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(
            "https://www.google.com/eq/1omg",
            found.0.as_str(),
            "Failed found {}",
            found.0
        );
    }

    #[test]
    fn can_find_url_4() {
        const DAT: &[u8] = b"test text my friend, whats up? (url: 127.0.0.1:80/eq/1omg!) whaaat?";
        let found = extract_possible_urls(RobustUtf8Reader::new(DAT.to_vec().reader())).unwrap();
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(
            "127.0.0.1:80/eq/1omg",
            found.0.as_str(),
            "Failed found {}",
            found.0
        );
    }

    static ENCODINGS: &'static [&'static Encoding] = &[
        UTF_8,
        BIG5,
        EUC_JP,
        EUC_KR,
        GB18030,
        GBK,
        IBM866,
        SHIFT_JIS,
        ISO_8859_2,
        ISO_8859_3,
        ISO_8859_4,
        ISO_8859_5,
        ISO_8859_6,
        ISO_8859_7,
        ISO_8859_8,
        ISO_8859_8_I,
        ISO_8859_10,
        ISO_8859_13,
        ISO_8859_14,
        ISO_8859_15,
        ISO_8859_16,
        ISO_2022_JP,
        WINDOWS_874,
        WINDOWS_1250,
        WINDOWS_1251,
        WINDOWS_1252,
        WINDOWS_1253,
        WINDOWS_1256,
        WINDOWS_1254,
        WINDOWS_1255,
        WINDOWS_1257,
        WINDOWS_1258,
        KOI8_R,
        KOI8_U,
        X_MAC_CYRILLIC
    ];

    #[test]
    fn test_encoding_method(){
        let x = ['A'..='Z', 'a'..='z', '0'..='9'].into_iter().flatten().collect::<String>().add("^,.-;:_+*/\\[](){}?~#'\"");
        for enc in ENCODINGS {
            let (a, b, c) = enc.encode(&x);
            assert_eq!(b, *enc);
            let mut y = String::new();
            for value in RobustUtf8Reader::new(a.reader()).map_ok(|value| value.ch) {
                match value {
                    Ok(value) => {
                        y.push(value);
                    }
                    Err(err) => {
                        y.push('°')
                    }
                }
            }
            println!("{} = \"{}\" == \"{}\"", x == y, x, y, );
        }
        println!("-----");
        let x = String::from_utf8((0..128u8).collect_vec()).unwrap();
        for enc in ENCODINGS {
            let (a, b, c) = enc.encode(&x);
            assert_eq!(b, *enc);
            let mut y = String::new();
            for value in RobustUtf8Reader::new(a.reader()).map_ok(|value| value.ch) {
                match value {
                    Ok(value) => {
                        y.push(value);
                    }
                    Err(err) => {
                        y.push('°')
                    }
                }
            }
            println!("{}: {} = {:?} == {:?}", enc.name(), x == y, x, y);
            // println!("{}: {} = {:?} == {:?}", enc.name(), x[(b' ' as usize)..(b'~' as usize)] == y[(b' ' as usize)..(b'~' as usize)], x[(b' ' as usize)..(b'~' as usize)].to_string(), y[(b' ' as usize)..(b'~' as usize)].to_string(), );
        }

        println!("{}..{} :: {}", (b' ' as usize), (b'~' as usize), ENCODINGS.len());
        println!("{}", (b'\r' as usize));
        println!("{}", (b'\n' as usize));
    }

    #[test]
    fn test_java_binary_code_extraction(){
        const TEST_DATA: &[u8] = include_bytes!("../../testdata/samples/Main.class");

        let x = extract_possible_urls(RobustUtf8Reader::new(TEST_DATA.reader())).expect("This should be working");
        for value in x {
            println!("{:?}", value)
        }
    }

    #[test]
    fn test_different_encodings() {
        const TEST_DATA: &str = include_str!("../../testdata/samples/Amazon.html");
      
        for encoding in ENCODINGS.iter().cloned() {
            let (content, used_enc, _) = encoding.encode(TEST_DATA);
            assert_eq!(
                encoding,
                used_enc,
                "The used encoding {} differs from the expected one {}",
                used_enc.name(),
                encoding.name()
            );
            let extracted = extract_possible_urls(RobustUtf8Reader::new(content.as_ref()));
            let mut collected = HashSet::new();
            match extracted {
                Ok(value) => {
                    let len = value.len();
                    collected.extend(value.into_iter().map(|value| value.0));
                    println!("OK:  {}: {} | {}", encoding.name(), len, collected.len());
                }
                Err(err) => {
                    println!("ERR: {}: {}", encoding.name(), err);
                }
            }
        }
    }

    #[test]
    fn test_class_file(){
        const DATA: &[u8] = include_bytes!("../../testdata/samples/Main.class");

        let found = extract_possible_urls(RobustUtf8Reader::new(DATA.reader())).unwrap();
        for (x, y) in found {
            println!("{x} - {y:?}")
        }
    }
}
