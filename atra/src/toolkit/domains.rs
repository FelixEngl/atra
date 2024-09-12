use case_insensitive_string::CaseInsensitiveString;
use psl::Domain;
use url::Url;

/// Get the domain name from the [url] as [CaseInsensitiveString].
/// Returns None if there is no domain
pub fn domain_name(url: &Url) -> Option<CaseInsensitiveString> {
    domain_name_raw(url).map(|value| CaseInsensitiveString::new(value.as_bytes()))
}


/// Get the raw domain name from [url]
/// Returns None if there is no domain
pub fn domain_name_raw(url: &Url) -> Option<Domain> {
    psl::domain(url.host_str()?.as_bytes())
}


