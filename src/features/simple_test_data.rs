use std::collections::HashMap;
use std::fmt::Write;
use serde::Serialize;

pub fn write_test_data<W: Write, E: Serialize>(writer: &mut W, entry: E) -> std::io::Result<()> {

}