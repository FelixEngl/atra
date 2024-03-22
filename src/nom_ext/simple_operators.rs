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

use nom::{InputLength, IResult};
use nom::character::complete::multispace0;
use nom::combinator::{fail, rest_len};
use nom::error::ParseError;
use nom::Parser;
use nom::sequence::delimited;

// pub fn is_empty<T: InputLength, E: ParseError<T>>(value: T) -> IResult<T, bool, E> {
//     rest_len(value).map(|(cont, ct)| (cont, ct == 0))
// }

pub fn is_empty_or_fail<T: InputLength+Clone, E: ParseError<T>>(value: T) -> IResult<T, T, E> {
    match rest_len(value) {
        Ok((cont, ct)) => {
            if ct == 0 {
                Ok((cont.clone(), cont))
            } else {
                fail(cont)
            }
        }
        Err(err) => Err(err)
    }
}


/// Something surrounded by whitespaces
pub fn ws<'a, O, E: ParseError<&'a str>, F: Parser<&'a str, O, E>>(
    f: F,
) -> impl Parser<&'a str, O, E> {
    delimited(multispace0, f, multispace0)
}