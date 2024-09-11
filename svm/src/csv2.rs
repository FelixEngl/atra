use std::io;
use std::marker::PhantomData;
use csv::{Reader, StringRecord, StringRecordsIntoIter};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use crate::classifier::TrainDataEntry;

/// An entry of a train csv
#[derive(Debug, Deserialize)]
pub struct CsvTrainModelEntry {
    pub is_class: bool,
    pub text: String
}

impl TrainDataEntry for CsvTrainModelEntry {
    fn get_label(&self) -> f64 {
        if self.is_class { 1.0 } else { -1.0 }
    }

    fn get_text(&self) -> &str {
        &self.text
    }
}

pub struct CsvProvider<T, R> {
    header: StringRecord,
    string_records_iter: StringRecordsIntoIter<R>,
    _produces: PhantomData<T>
}

impl<T, R> CsvProvider<T, R> {
    unsafe fn new_(header: StringRecord, string_records_iter: StringRecordsIntoIter<R>) -> Self {
        Self { header, string_records_iter, _produces: PhantomData }
    }
}

impl<T, R> CsvProvider<T, R> where R: io::Read {
    pub fn new(mut string_records_iter: Reader<R>) -> io::Result<Self> {
        let header = string_records_iter.headers()?;
        unsafe {
            Ok(
                Self::new_(
                    header.clone(),
                    string_records_iter.into_records()
                )
            )
        }
    }
}

impl<T, R> Iterator for CsvProvider<T, R> where T: DeserializeOwned, R: io::Read {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.string_records_iter.next()?.ok()?;
        next.deserialize(Some(&self.header)).ok()
    }
}