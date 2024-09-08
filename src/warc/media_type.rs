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

use std::borrow::{Borrow, BorrowMut};
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut, RangeInclusive};
use std::str;

use compact_str::{CompactString, ToCompactString};
use const_format::concatcp;
use itertools::Itertools;
use mime::{Mime, Params};
use nom::{AsChar, InputTakeAtPosition, IResult};
use nom::branch::alt;
use nom::bytes::complete::take_till;
use nom::bytes::streaming::{escaped, tag};
use nom::character::streaming::{char as streaming_char, line_ending, multispace0};
use nom::combinator::{map, map_res};
use nom::error::{ErrorKind, FromExternalError, ParseError, VerboseError};
use nom::multi::many_till;
use nom::sequence::{delimited, pair, preceded, separated_pair};
use crate::nom_ext::simple_operators::is_empty_or_fail;


/// Parses some bytes to a [MediaType]
pub fn parse_media_type<const COMPLETE_DATA: bool>(b: &[u8]) -> IResult<&[u8], MediaType> {
    map(
        pair(parse_types, parse_parameters::<COMPLETE_DATA>),
        |value| {
            MediaType {
                type_: value.0.0,
                sub_type: value.0.1,
                parameters: value.1
            }
        }
    )(b)
}

/// Describes the media type of some content
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct MediaType {
    type_: CompactString,
    sub_type: CompactString,
    parameters: Option<Parameters>
}

impl MediaType {
    pub fn new(
        type_: impl ToCompactString,
        sub_type: impl ToCompactString,
        parameters: Option<Parameters>
    ) -> Self {
        Self {
            type_: type_.to_compact_string(),
            sub_type: sub_type.to_compact_string(),
            parameters
        }
    }

    pub fn from_mime(mime: &Mime) -> Self {
        let params = Parameters::from_params(mime.params());
        Self {
            type_: mime.type_().to_compact_string(),
            sub_type: mime.subtype().to_compact_string(),
            parameters: (!params.is_empty()).then_some(params)
        }
    }
}

impl Display for MediaType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(ref params) = self.parameters {
            write!(f, "{}/{}{}", self.type_, self.sub_type, params)
        } else {
            write!(f, "{}/{}", self.type_, self.sub_type)
        }
    }
}


/// A vec of parameters
#[derive(Debug, Clone, Eq)]
#[repr(transparent)]
pub struct Parameters(Vec<Parameter>);

impl Parameters {
    pub fn from_params(params: Params) -> Self {
        Self(
            params
                .map(
                    |(k, v)|
                        Parameter::new(k.to_compact_string(), v.to_compact_string())
                )
                .collect_vec()
        )
    }
}

impl Display for Parameters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for value in &self.0 {
            write!(f, ";{}", value)?
        }
        Ok(())
    }
}

impl Hash for Parameters {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl PartialOrd<Self> for Parameters {
    #[inline] fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        PartialOrd::partial_cmp(&self.0, &other.0)
    }
}

impl Ord for Parameters {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.0, &other.0)
    }
}

impl PartialEq for Parameters {
    #[inline] fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }

    #[inline] fn ne(&self, other: &Self) -> bool {
        self.0.ne(&other.0)
    }
}

impl Deref for Parameters {
    type Target = Vec<Parameter>;

    #[inline] fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Parameters {
    #[inline] fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Borrow<Vec<Parameter>> for Parameters {
    #[inline] fn borrow(&self) -> &Vec<Parameter> {
        &self
    }
}

impl BorrowMut<Vec<Parameter>> for Parameters {
    #[inline] fn borrow_mut(&mut self) -> &mut Vec<Parameter> {
        &mut self.0
    }
}

#[derive(Debug, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Parameter  {
    name: CompactString,
    value: CompactString
}

impl Parameter {
    pub fn new(name: CompactString, value: CompactString) -> Self {
        Self { name, value }
    }
}

impl Display for Parameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.value.contains(|c| DELIMITERS_WS_SP.contains(c)) {
            write!(
                f,
                "{}=\"{}\"",
                self.name,
                self.value.escape_default()
            )
        } else {
            write!(
                f,
                "{}={}",
                self.name,
                self.value
            )
        }

    }
}


// ##### Parsing starts here #####


const DELIMITERS: &str = "(),/:;<=>?@[\\]{}\"";
const DELIMITERS_WS: &str = concatcp!(DELIMITERS, " \t");
const DELIMITERS_WS_SP: &str = concatcp!(DELIMITERS_WS, "\r\n");

const VCHAR_VALUES: RangeInclusive<char> = (0x21u8 as char)..=(0x7Eu8 as char);
const OBS_TEXT_VALUES: RangeInclusive<char> = (0x80u8 as char)..=(0xFFu8 as char);

// https://datatracker.ietf.org/doc/html/rfc5234#appendix-B.1
// fn vchar0<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
//     where
//         T: InputTakeAtPosition,
//         <T as InputTakeAtPosition>::Item: AsChar,
// {
//     input.split_at_position(|item| ((0x21u8 as char)..=(0x7Eu8 as char)).contains(item))
// }
fn vchar1<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    input.split_at_position1(|item| VCHAR_VALUES.contains(&item.as_char()), ErrorKind::Fail)
}

// fn obs_text0<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
//     where
//         T: InputTakeAtPosition,
//         <T as InputTakeAtPosition>::Item: AsChar,
// {
//     input.split_at_position(|item| ((0x80u8 as char)..=(0xFFu8 as char)).contains(item))
// }
fn obs_text1<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    input.split_at_position1(|item| OBS_TEXT_VALUES.contains(&item.as_char()), ErrorKind::Fail)
}

fn qdtext1<T: Debug, E: ParseError<T>>(input: T) -> IResult<T, T, E>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar + Debug + Clone,
{
    input.split_at_position1(|item|
                                 {
                                     let char = item.clone().as_char();
                                     !('\t' == char
                                         || ' ' == char
                                         || (0x21u8 as char) == char
                                         || ((0x23u8 as char)..=(0x5Bu8 as char)).contains(&char)
                                         || ((0x5Du8 as char)..=(0x7Eu8 as char)).contains(&char)
                                         || ((0x80u8 as char)..=(0xFFu8 as char)).contains(&char))
                                 }
                             , ErrorKind::Fail)
}

fn token1<I, Error: ParseError<I> + Debug>(value: I) -> IResult<I, I, Error> where
    I: InputTakeAtPosition + Debug,
    <I as InputTakeAtPosition>::Item: AsChar
{
    take_till::<_, I, Error>(|c| DELIMITERS_WS.contains(c.as_char()))(value)
}

fn parse_types(b: &[u8]) -> IResult<&[u8], (CompactString, CompactString)> {
    separated_pair(
        map_res(token1, |value| str::from_utf8(value).map(|it| it.to_compact_string())),
        streaming_char('/'),
        map_res(token1, |value| str::from_utf8(value).map(|it| it.to_compact_string()))
    )(b)
}


fn parse_parameters<const COMPLETE_DATA: bool>(b: &[u8]) -> IResult<&[u8], Option<Parameters>> {
    fn parse_separator(b: &[u8]) -> IResult<&[u8], &[u8]> {
        delimited(multispace0, tag(b";"), multispace0)(b)
    }

    fn parse_entry(b: &[u8]) -> IResult<&[u8], Parameter> {
        map(
            separated_pair(
                map_res(token1, |value| str::from_utf8(value).map(|it| it.to_compact_string())),
                tag(b"="),
                alt((
                    map_res(
                        delimited(
                            tag(b"\""),
                            escaped(
                                qdtext1,
                                '\\',
                                alt(
                                    (
                                        tag(b"\""),
                                        tag(b" "),
                                        tag(b"t"),
                                        tag(b"r"),
                                        tag(b"n"),
                                        vchar1,
                                        obs_text1,
                                    )
                                )
                            ),
                            tag(b"\"")
                        ),
                        |value| match str::from_utf8(value) {
                            Ok(result) => {
                                match unescaper::unescape(result) {
                                    Ok(result) => {
                                        Ok(result.to_compact_string())
                                    }
                                    Err(err) => {
                                        Err(
                                            VerboseError::from_external_error(
                                                value, ErrorKind::EscapedTransform, err
                                            )
                                        )
                                    }
                                }
                            }
                            Err(err) => {
                                Err(
                                    VerboseError::from_external_error(
                                        value, ErrorKind::EscapedTransform, err
                                    )
                                )
                            }
                        }
                    ),
                    map_res(token1, |value| str::from_utf8(value).map(|it| it.to_compact_string()))
                ))
            ),
            |value| Parameter {name: value.0, value: value.1}
        )(b)
    }

    let target = if COMPLETE_DATA {
        |value| alt((
            is_empty_or_fail,
            line_ending,
        ))(value)
    } else {
        |dat| line_ending(dat)
    };

    map(
        many_till(
            preceded(
                parse_separator,
                parse_entry
            ),
            target
        ),
        |(params, _)| {
           if params.is_empty() { None } else {Some(Parameters(params))}
        }
    )(b)
}



#[cfg(test)]
mod test {
    use nom::branch::alt;
    use nom::bytes::streaming::{escaped, tag};
    use nom::IResult;
    use nom::sequence::delimited;

    use crate::warc::media_type::{obs_text1, parse_media_type, parse_parameters, parse_types, qdtext1, vchar1};

    #[test]
    fn can_parse(){
        println!("1 {:?}", parse_types(b"text/html"));
        println!("2 {:?}", parse_types(b"text/html;charset=utf-8"));
        println!("3 {:?}", parse_parameters::<true>(b";charset=utf-8"));
        println!("4 {:?}", parse_media_type::<true>(b"text/html;charset=UTF-8"));
        println!("5 {:?}", parse_media_type::<true>(b"Text/HTML;Charset=\"utf-8\""));
        println!("6 {:?}", parse_media_type::<true>(b"text/html; charset=\"utf-8\""));
        println!("7 {:?}", parse_parameters::<true>(b";charset=\"utf-8\""));

        let x: IResult<&[u8], &[u8]> = delimited(
            tag(b"\""),
            escaped(
                qdtext1,
                '\\',
                alt(
                    (
                        tag(b" "),
                        tag(b"t"),
                        tag(b"r"),
                        tag(b"n"),
                        vchar1,
                        obs_text1,
                    )
                )
            ),
            tag(b"\"")
        )(b"\"utf-8\"".as_slice());

        println!("{x:?}")
    }
}