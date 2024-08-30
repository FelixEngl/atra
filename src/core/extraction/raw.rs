use memchr::memmem::Prefilter;

/// Extracts links from raw bytes.
pub fn extract_possible_urls(value: &[u8]) -> Vec<&[u8]> {
    let mut links = Vec::new();

    let finder = memchr::memmem::FinderBuilder::new()
        .prefilter(Prefilter::Auto)
        .build_forward(b"http");

    for start_of_http in finder.find_iter(value) {
        let candidate = &value[start_of_http..];
        let prune_start = if candidate.starts_with(b"https://") {
            8usize
        } else if candidate.starts_with(b"http://") {
            7usize
        } else {
            continue;
        };

        let possible_end_of_url = if let Some(possible_end_of_url) = memchr::memchr3(b' ', b'"', b'\'', candidate) {
            possible_end_of_url
        } else if let Some(possible_end_of_url) = memchr::memchr3(b'\t', b'\r', b'\n', candidate) {
            possible_end_of_url
        } else {
            candidate.len()
        };

        let mut candidate = &candidate[..possible_end_of_url];
        if let Some(start_of_important_slash) = memchr::memchr(b'/', &candidate[prune_start..]) {
            let start_of_important_slash = start_of_important_slash + prune_start;

            let target = match candidate[candidate.len() - 1] {
                b')' => Some(b'('),
                b']' => Some(b'['),
                b'}' => Some(b'{'),
                _ => None,
            };
            if let Some(target) = target {
                if memchr::memrchr(target, candidate).is_none() {
                    candidate = &candidate[..candidate.len() - 1];
                }
            }
            if let Some(_) = psl::suffix(&candidate[..start_of_important_slash]) {
                links.push(candidate);
            }
        }
    }

    return links
}

#[cfg(test)]
mod test {
    use itertools::Itertools;
    use crate::core::extraction::raw::extract_possible_urls;

    #[test]
    fn can_find_url_1() {
        const DAT: &[u8] = b"test text my friend, whats up? http://www.google.com/eq/1 omg!";
        let found = extract_possible_urls(DAT);
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(found, b"http://www.google.com/eq/1", "Failed found {}", String::from_utf8(found.to_vec()).unwrap());
    }

    #[test]
    fn can_find_url_2() {
        const DAT: &[u8] = b"test text my friend, whats up? https://www.google.com/eq/1omg!";
        let found = extract_possible_urls(DAT);
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(found, b"https://www.google.com/eq/1omg!", "Failed found {}", String::from_utf8(found.to_vec()).unwrap());
    }

    #[test]
    fn can_find_url_3() {
        const DAT: &[u8] = b"test text my friend, whats up? (url: https://www.google.com/eq/1omg!) whaaat?";
        let found = extract_possible_urls(DAT);
        assert!(!found.is_empty());
        let found = found.into_iter().exactly_one().unwrap();
        assert_eq!(found, b"https://www.google.com/eq/1omg!", "Failed found {}", String::from_utf8(found.to_vec()).unwrap());
    }
}