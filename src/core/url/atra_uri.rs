use std::fmt::{Display, Formatter};
use std::str::FromStr;
use case_insensitive_string::CaseInsensitiveString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use crate::core::url::cleaner::AtraUrlCleaner;



/// A separated type for URL to prepare for supporting different kind of URLs
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
pub enum AtraUri {
    Url(Url)
}



#[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
pub enum HostComparisonError {
    /// Only for [AtraUri::Url], returned when there is no host.
    #[error("The left has {} and right has {}!", if *(.left_has_host) {"host"} else {"no host"}, if *(.right_has_host) {"host"} else {"no host"})]
    NoHost {
        left_has_host: bool,
        right_has_host: bool
    }
}


/// Errors when converting something to an [AtraUri]
#[derive(Debug, Clone, Error)]
pub enum ParseError {
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError)
}

impl AtraUri {
    pub fn with_base<U: AsRef<str>>(base: &Self, target: U) -> Result<Self, ParseError> {
        let target = target.as_ref();
        match base {
            AtraUri::Url(base) => {
                Ok(AtraUri::Url(Url::options().base_url(Some(base)).parse(target)?))
            }
        }
    }

    /// Returns the scheme of the underlying url
    pub fn scheme(&self) -> &str {
        match self { AtraUri::Url(value) => {value.scheme()} }
    }

    #[inline(always)]
    pub fn clean<C: AtraUrlCleaner>(&mut self, cleaner: C) {
        cleaner.clean(self)
    }

    pub fn host_str(&self) -> Option<&str> {
        match self { AtraUri::Url(value) => {value.host_str()} }
    }

    pub fn domain(&self) -> Option<&str> {
        match self { AtraUri::Url(value) => {value.domain()} }
    }

    pub fn path(&self) -> Option<&str> {
        match self { AtraUri::Url(value) => {Some(value.path())} }
    }

    pub fn same_host(&self, other: &Self) -> bool {
        match self {
            AtraUri::Url(a) => {
                match other {
                    AtraUri::Url(b) => {
                        a.host() == b.host()
                    }
                }
            }
        }
    }

    pub fn same_host_url(&self, other: &Url) -> bool {
        match self {
            AtraUri::Url(a) => {
                a.host() == other.host()
            }
        }
    }

    /// Returns the name of the domain without any suffix.
    /// Cleanup depends on the public suffix list.
    pub fn domain_name(&self) -> Option<CaseInsensitiveString> {
        match self {
            AtraUri::Url(value) => {
                Some(
                    CaseInsensitiveString::new(
                        psl::domain(
                            value.host_str()?.as_bytes()
                        )?.as_bytes()
                    )
                )
            }
        }
    }

    /// Returns it as string
    pub fn as_str(&self) -> &str {
        match self {
            AtraUri::Url(value) => {
                value.as_str()
            }
        }
    }

    /// Compares the host of two [AtraUri]s. Fails if they are somewhat not comparable.
    pub fn compare_hosts(&self, other: &Self) -> Result<bool, HostComparisonError> {
        #[inline(always)]
        fn compare_url(a: &Url, b: &Url) -> Result<bool, HostComparisonError> {
            if let Some(host_a) = a.host_str() {
                if let Some(host_b) = b.host_str() {
                    Ok(host_a.eq_ignore_ascii_case(host_b))
                } else {
                    Err(
                        HostComparisonError::NoHost {
                            left_has_host: true,
                            right_has_host: false
                        }
                    )
                }
            } else {
                Err(
                    HostComparisonError::NoHost {
                        left_has_host: false,
                        right_has_host: b.has_host()
                    }
                )
            }
        }

        match self {
            AtraUri::Url(a) => {
                match other {
                    AtraUri::Url(b) => {
                        compare_url(a, b)
                    }
                }
            }
        }
    }

    pub fn as_url(&self) -> Option<&Url> {
        match self { AtraUri::Url(value) => {Some(value)} }
    }

    pub fn as_mut_url(&mut self) -> Option<&mut Url> {
        match self { AtraUri::Url(value) => {Some(value)} }
    }
}

impl FromStr for AtraUri {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.parse::<Url>()?.into())
    }
}

impl From<Url> for AtraUri {
    fn from(value: Url) -> Self {
        AtraUri::Url(value)
    }
}

impl Display for AtraUri {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self { AtraUri::Url(value) => {Display::fmt(value, f)} }
    }
}


#[cfg(test)]
mod test {
    use url::Url;
    use crate::core::url::atra_uri::{AtraUri, HostComparisonError};

    #[test]
    fn spaces_do_not_matter() {
        let a: AtraUri = "https://www.example.com/".parse().expect("Expected a sucessfull parse!");
        let b: AtraUri = "  https://www.example.com/  ".parse().expect("Expected a sucessfull parse!");
        assert_eq!(a, b);
        assert_eq!(a.as_str(), b.as_str());
    }


    fn old_impl(base: &Url, url: &Url) -> Result<bool, HostComparisonError> {
        if let Some(host) = url.host_str() {
            if let Some(base_host) = base.host_str() {
                Ok(host.eq_ignore_ascii_case(base_host))
            } else {
                Err(
                    HostComparisonError::NoHost {
                        left_has_host: true,
                        right_has_host: false
                    }
                )
            }
        } else {
            Err(
                HostComparisonError::NoHost {
                    left_has_host: false,
                    right_has_host: base.has_host()
                }
            )
        }
    }

    #[test]
    fn has_same_behaviour_than_old_impl(){
        let base1 = "https://www.siemens.com/".parse::<Url>().expect("Success!");
        let base2 = "https://127.0.0.1:8081/whaat".parse::<Url>().expect("Success!");
        let base3 = "file://data/simpen.txt".parse::<Url>().expect("Success!");
        let other1 = "https://www.siemens.com/lookup".parse::<Url>().expect("Success!");
        let other2 = "https://www.google.com/lookup".parse::<Url>().expect("Success!");
        let other3 = "https://127.0.0.1:8080/lookup".parse::<Url>().expect("Success!");
        let other4 = "file://data/simpen.txt".parse::<Url>().expect("Success!");
        let other5 = "file://data/readme.md".parse::<Url>().expect("Success!");
        let bases = [base1, base2, base3];
        let others = [other1, other2, other3, other4, other5];
        for base in &bases{
            for other in &others {
                let old = old_impl(base, other);
                let new =  AtraUri::from(other.clone()).compare_hosts(&base.clone().into());
                assert_eq!(old, new, "Failed {old:?} {new:?} for base={base}, other={other}");
            }
        }
    }
}
