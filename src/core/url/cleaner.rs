use sealed::sealed;
use strum::{VariantArray};
use url::Url;
use crate::core::url::atra_uri::AtraUri;

#[sealed]
pub trait AtraUrlCleaner  {
    /// Cleans the [AtraUri]
    fn clean(&self, url: &mut AtraUri) {
        match url {
            AtraUri::Url(value) => {
                self.clean_url(value)
            }
        }
    }

    /// Cleans an url
    fn clean_url(&self, url: &mut Url);
}

#[sealed]
impl AtraUrlCleaner for &[SingleUrlCleaner] {
    fn clean_url(&self, url: &mut Url) {
        for value in self.iter() {
            value.clean_url(url)
        }
    }
}

#[sealed]
impl<const SIZE: usize> AtraUrlCleaner for [SingleUrlCleaner; SIZE] {
    fn clean_url(&self, url: &mut Url) {
        for value in self.iter() {
            value.clean_url(url)
        }
    }
}

#[derive(Debug, Copy, Clone, VariantArray)]
pub enum SingleUrlCleaner {
    Fragment,
    Query,
    Path,
    Port,
    Password,
    Username
}

#[sealed]
impl AtraUrlCleaner for SingleUrlCleaner {
    fn clean_url(&self, url: &mut Url) {
        match self {
            SingleUrlCleaner::Fragment => {
                url.set_fragment(None)
            }
            SingleUrlCleaner::Query => {
                url.set_query(None)
            }
            SingleUrlCleaner::Path => {
                url.set_path("")
            }
            SingleUrlCleaner::Port => {
                let _ = url.set_port(None);
            }
            SingleUrlCleaner::Password => {
                let _ = url.set_password(None);
            }
            SingleUrlCleaner::Username => {
                let _ = url.set_username("");
            }
        }
    }
}
