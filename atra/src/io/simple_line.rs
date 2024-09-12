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

use std::io;
use std::io::{BufRead, BufReader, Cursor, Lines};

use itertools::{Itertools, Position};

/// A struct implementing this trait supports beeing read as simple lines
pub trait SupportsSimpleLineReader {
    /// Returns a simple line reader
    fn to_simple_line_reader(self) -> SimpleLinesReader<Self>
    where
        Self: BufRead + Sized;
}

impl<T: BufRead> SupportsSimpleLineReader for T {
    fn to_simple_line_reader(self) -> SimpleLinesReader<Self>
    where
        Self: BufRead + Sized,
    {
        SimpleLinesReader::new(self.lines())
    }
}

/// A simple line based file-format with a line-based .
/// - Starting a line with # indicates a comment.
/// -
/// - To start a line with '#' use '\#' at the start. Otherwise using # is possible without any escaping.
/// - Empty lines are not allowed
/// - If multiple lines are needed, each line ending with '\' is considered concatenated with the following by '\n'.
///   If the line ends with '\\' the line ends with \ and not with '\n'.
/// - All other '\'  are used as simple characters.
pub struct SimpleLinesReader<B> {
    buf: Lines<B>,
    err_state: bool,
}

impl<B: BufRead> SimpleLinesReader<B> {
    pub fn new(buf: Lines<B>) -> Self {
        Self {
            buf,
            err_state: false,
        }
    }
}

impl<T: AsRef<[u8]>> SimpleLinesReader<BufReader<Cursor<T>>> {
    #[cfg(test)]
    pub fn with_cursor(value: T) -> Self {
        Self::new(BufReader::new(Cursor::new(value)).lines())
    }
}

impl<B: BufRead> Iterator for SimpleLinesReader<B> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.err_state {
            return None;
        }
        loop {
            match self.buf.next() {
                Some(Ok(line)) => {
                    if line.is_empty() || !(line.starts_with("\\#") || line.ends_with('\\')) {
                        return Some(Ok(line));
                    }
                    if line.starts_with('#') {
                        continue;
                    } else {
                        let mut collector = line;
                        if collector.starts_with("\\#") {
                            collector.remove(0);
                        }
                        if collector.ends_with('\\') {
                            collector.pop();
                            let needs_processing = collector.ends_with("\\\\");
                            if needs_processing {
                                collector.pop();
                            }
                            if needs_processing || !collector.ends_with('\\') {
                                while let Some(next) = self.buf.next() {
                                    match next {
                                        Ok(mut next) => {
                                            let starts_with_ignored_comment =
                                                next.starts_with("\\#");
                                            if next.ends_with('\\') {
                                                next.pop();
                                                let needs_processing = next.ends_with("\\\\");
                                                if needs_processing {
                                                    next.pop();
                                                }
                                                if needs_processing || !next.ends_with('\\') {
                                                    collector.push('\n');
                                                    collector.push_str(
                                                        &next[(starts_with_ignored_comment
                                                            as usize)..],
                                                    );
                                                    continue;
                                                }
                                            }
                                            collector.push('\n');
                                            collector.push_str(
                                                &next[(starts_with_ignored_comment as usize)..],
                                            );
                                            break;
                                        }
                                        Err(err) => {
                                            self.err_state = true;
                                            return Some(Err(err));
                                        }
                                    }
                                }
                            }
                        }
                        return Some(Ok(collector));
                    }
                }
                x @ Some(Err(_)) => {
                    self.err_state = true;
                    return x;
                }
                None => return None,
            }
        }
    }
}

fn convert_to_simple_line_format_string<T: AsRef<str>>(value: T) -> String {
    let s = value.as_ref();
    let mut next = String::with_capacity(s.len());
    if s.contains('\n') {
        let lines = s.lines();
        for (pos, line) in lines.with_position() {
            if line.starts_with('#') {
                next.push('\\');
            }
            next.push_str(line);
            if line.ends_with('\\') {
                next.push('\\')
            }
            match pos {
                Position::Last => {}
                _ => {
                    next.push_str("\\\n");
                }
            }
        }
    } else {
        if s.starts_with('#') {
            next.push('\\');
        }
        next.push_str(s);
        if s.ends_with('\\') {
            next.push('\\')
        }
    }
    return next;
}

pub trait SupportsSimpleLineMapper {
    fn to_simple_lines(self) -> impl Iterator<Item = String>;
}

impl<I, T> SupportsSimpleLineMapper for I
where
    I: Iterator<Item = T>,
    T: AsRef<str>,
{
    /// Converts an iterator for some strings into a string formatted as a simple line entry.
    fn to_simple_lines(self) -> impl Iterator<Item = String> {
        self.map(convert_to_simple_line_format_string)
    }
}

#[cfg(test)]
mod tests {
    use crate::io::simple_line::{SimpleLinesReader, SupportsSimpleLineMapper};
    use itertools::Itertools;

    #[test]
    pub fn can_serialize_and_deserialize() {
        let data = vec![
            "Hallo Welt\nHier bin ich!",
            "# Also so kann \\ das nichts werden!\\",
            "Dieser satz\nmacht keinen\\\n# sinn!!!",
            "# Aber\nhallo!\n#wie viel wollen wir\ntun\n\noder so?",
        ];
        // println!("{}", data.into_iter().to_simple_line_iter().join("\n"));
        let dat = data.clone().into_iter().to_simple_lines().join("\n");
        println!("{dat}");
        let result = SimpleLinesReader::with_cursor(dat)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        for (a, b) in data.into_iter().zip(result.into_iter()) {
            assert_eq!(a.to_string(), b)
        }
    }
}
