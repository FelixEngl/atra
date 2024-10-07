// Copyright 2024. Felix Engl
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

use std::fmt::{Display, Formatter};
use std::sync::Arc;
use rocksdb::{DBIteratorWithThreadMode, DBWithThreadMode, Direction, Error, IteratorMode, MultiThreaded};
use crate::contexts::local::LocalContext;
use crate::crawl::{SlimCrawlResult};
use crate::url::AtraUri;
use crate::warc_ext::ReaderError;

#[derive(Clone, Debug)]
#[repr(transparent)]
pub(crate) struct SlimEntry(pub(crate) Arc<(AtraUri, SlimCrawlResult)>);

impl Display for SlimEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0.as_ref().0, f)
    }
}


impl From<(Box<[u8]>, Box<[u8]>)> for SlimEntry {
    fn from((k, v): (Box<[u8]>, Box<[u8]>)) -> Self {
        let k: AtraUri = String::from_utf8_lossy(k.as_ref()).parse().unwrap();
        let v: SlimCrawlResult = bincode::deserialize(v.as_ref()).unwrap();
        Self(Arc::new((k, v)))
    }
}


pub(crate) struct ControlledIterator<'a> {
    context: &'a LocalContext,
    iter: DBIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>,
    selection_size: usize,
    selection: Vec<SlimEntry>,
    selected: Option<(usize, AtraUri, SlimCrawlResult)>,
    direction: Direction,
    end_reached: bool
}


impl<'a> ControlledIterator<'a> {
    pub fn new(context:&'a LocalContext, selection_size: usize) -> Result<Self, Vec<Error>> {
        let iter = context.crawl_db().iter(IteratorMode::Start);
        let mut new = Self {
            context,
            iter,
            selection_size,
            selection: Vec::with_capacity(selection_size),
            selected: None,
            direction: Direction::Forward,
            end_reached: false
        };
        new.next()?;
        Ok(new)
    }

    pub fn set_selection_size(&mut self, selection_size: usize) {
        self.selection_size = selection_size;
    }

    fn load_next(&mut self, direction: Direction) -> Result<usize, Vec<Error>> {
        let mode = match direction {
            Direction::Forward => {
                if matches!(self.direction, Direction::Reverse){
                    if let Some(last) = self.selection.last() {
                        if self.end_reached {
                            Some(IteratorMode::Start)
                        } else {
                            Some(IteratorMode::From(last.0.as_ref().0.as_bytes(), Direction::Forward))
                        }
                    } else {
                        None
                    }
                } else {
                    if self.end_reached {
                        return Ok(0)
                    }
                    None
                }
            }
            Direction::Reverse => {
                if matches!(self.direction, Direction::Forward){
                    if let Some(last) = self.selection.first() {
                        if self.end_reached {
                            Some(IteratorMode::End)
                        } else {
                            Some(IteratorMode::From(last.0.as_ref().0.as_bytes(), Direction::Reverse))
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };



        if let Some(mode) = mode {
            self.iter.set_mode(mode);
            self.direction = direction;
        }

        self.selection.clear();
        self.selected.take();
        let mut errors = Vec::with_capacity(1);

        while self.selection.len() < self.selection_size {
            if let Some(found) = self.iter.next() {
                match found {
                    Ok(value) => {
                        self.selection.push(value.into())
                    }
                    Err(err) => {
                        errors.push(err)
                    }
                }
            } else {
                break
            }
        }

        self.end_reached =  self.selection.len() != self.selection_size;

        match self.direction {
            Direction::Reverse => {
                self.selection.reverse()
            }
            _ => {}
        }

        if errors.is_empty() {
            Ok(self.selection.len())
        } else {
            Err(errors)
        }
    }

    pub fn current(&self) -> &[SlimEntry] {
        self.selection.as_slice()
    }

    pub fn next(&mut self) -> Result<Option<&[SlimEntry]>, Vec<Error>> {
        match self.load_next(Direction::Forward)?  {
            0 => Ok(None),
            _ => Ok(Some(self.selection.as_slice())),
        }
    }

    pub fn previous(&mut self) -> Result<Option<&[SlimEntry]>, Vec<Error>> {
        match self.load_next(Direction::Reverse)?  {
            0 => Ok(None),
            _ => Ok(Some(self.selection.as_slice())),
        }
    }

    pub fn select(&mut self, idx: usize) -> Result<Option<&(usize, AtraUri, SlimCrawlResult)>, ReaderError> {
        match self.selection.get(idx) {
            None => {
                return Ok(None)
            }
            Some(selected) => {
                let (uri, result) = selected.0.as_ref().clone();
                let result = (idx, uri, result);
                self.selected.replace(result);
                Ok(self.selected.as_ref())
            }
        }
    }

    pub fn current_selected(&self) -> Option<&(usize, AtraUri, SlimCrawlResult)> {
        self.selected.as_ref()
    }

    pub fn end_reached(&self) -> bool {
        self.end_reached
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn selection_size(&self) -> usize {
        self.selection_size
    }
}