use std::sync::LazyLock;
use mime::{Mime};



pub static APPLICATION_XML: LazyLock<Mime> = LazyLock::new(|| "application/xml".parse::<Mime>().unwrap());
pub static APPLICATION_RTF: LazyLock<Mime> = LazyLock::new(|| "application/rtf".parse::<Mime>().unwrap());
