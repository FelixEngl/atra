use crate::core::extraction::raw::extract_possible_urls;
use crate::core::page_type::PageType;
macro_rules! create_extractor_method {
    (
        return $return_type: ty;
        $(
            impl $name: ident $(| $alias: literal)*: {
                fn is_compatible(&$self1: ident, $context1:ident: &impl Context, $page1:ident: &ProcessedData<'_>) -> bool $block1: block
                async fn extract_links(&$self2: ident, $context2:ident: &impl Context, $page2:ident: &ProcessedData<'_>, $result2: ident: &mut ExtractorResult) $block2: block
            };
        )+
    ) => {
        use $crate::core::data_processing::ProcessedData;
        use $crate::core::contexts::Context;
        use $crate::core::extraction::extractor::ExtractorResult;
        use $crate::core::extraction::marker::ExtractorMethodHint;
        use $crate::core::extraction::marker::ExtractorMethodMeta;
        use $crate::core::extraction::links::ExtractedLink;
        use $crate::core::decoding::DecodedData;
        use enum_iterator::Sequence;
        use serde::{Deserialize, Serialize};


        #[derive(Sequence, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone)]
        pub enum ExtractorMethod {
            $(
                $(#[serde(alias = $alias)])*
                $name,
            )+
        }

        impl ExtractorMethod {
            $(
                paste::paste! {
                    #[allow(non_snake_case)]
                    #[inline(always)]
                    fn [<is_compatible_ $name>](&$self1, $context1: &impl Context, $page1: &ProcessedData<'_>) -> bool $block1
                }
            )+

            pub fn is_compatible(&self, context: &impl Context, page: &ProcessedData<'_>) -> bool {
                match self {
                    $(
                        ExtractorMethod::$name => {
                            paste::paste! {
                                self.[<is_compatible_ $name>](context, page)
                            }
                        }
                    )+
                }
            }

            $(
                paste::paste! {
                    #[allow(non_snake_case)]
                    #[inline(always)]
                    async fn [<extract_links_ $name>](&$self2, $context2: &impl Context, $page2: &ProcessedData<'_>, $result2: &mut ExtractorResult) -> $return_type $block2
                }
            )+

            pub async fn extract_links(&self, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) -> $return_type {
                match self {
                    $(
                        ExtractorMethod::$name => {
                            paste::paste! {
                                self.[<extract_links_ $name>](context, page, output).await
                            }
                        }
                    )+
                }
            }
        }
    };
}




create_extractor_method! {
    return Result<usize, ()>;
    impl HtmlV1 | "HTML_v1": {
        fn is_compatible(&self, _context: &impl Context, page: &ProcessedData<'_>) -> bool {
            page.0.page_type == PageType::HTML
        }

        async fn extract_links(&self, context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) {
            match &page.1 {
                DecodedData::InMemory{ result, .. } => {
                    match crate::core::extraction::html::extract_links(
                        &page.0.data.url,
                        result.as_str(),
                        context.configs().crawl().respect_nofollow,
                        context.configs().crawl().crawl_embedded_data,
                        context.configs().crawl().crawl_javascript,
                        context.configs().crawl().crawl_onclick_by_heuristic,
                    ) {
                        None => Ok(0),
                        Some((base, extracted, errors)) => {
                            if !errors.is_empty() {
                                if log::max_level() <= log::LevelFilter::Trace {
                                    let mut message = String::new();
                                    for err in errors {
                                        message.push_str(err.as_ref());
                                        message.push('\n');
                                    }
                                    log::trace!("Error parsing '{}'\n---START---\n{message}\n---END---\n", page.0.data.url)
                                }
                            }
                            let mut ct = 0usize;
                            let base_ref = base.as_ref();
                            for (origin, link) in extracted {
                                match ExtractedLink::pack(base_ref, &link, ExtractorMethodHint::new_with_meta(self.clone(), ExtractorMethodMeta::Html(origin))) {
                                    Ok(link) => {
                                        if link.is_not(base_ref) {
                                            if output.register_link(link) {
                                                ct += 1;
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        log::debug!("Was not able to parse link {} from html. Error: {}", link, error)
                                    }
                                }
                            }
                            Ok(ct)
                        }
                    }
                }
                DecodedData::OffMemory{ .. } => {Err(())}
                DecodedData::None => {Ok(0)}
            }
        }
    };

    impl JSV1 | "js_v1" | "JavaScript_v1" | "JS_v1": {
        fn is_compatible(&self, context: &impl Context, page: &ProcessedData<'_>)-> bool {
            context.configs().crawl().crawl_javascript && page.0.page_type == PageType::JavaScript
        }

        async fn extract_links(&self, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) {

            match &page.1 {
                DecodedData::InMemory{ result, .. } => {
                    let mut ct = 0usize;
                    for entry in crate::core::extraction::js::extract_links(result.as_str()) {
                        match ExtractedLink::pack(&page.0.get_page().url, entry.as_str(), ExtractorMethodHint::new_without_meta(self.clone())) {
                            Ok(link) => {
                                if output.register_link(link) {
                                    ct += 1;
                                }
                            }
                            Err(error) => {
                                log::debug!("Was not able to parse {} from javascript. Error: {}", entry, error)
                            }
                        }
                    }
                    Ok(ct)
                }
                DecodedData::OffMemory{ .. } => {Err(())}
                DecodedData::None => {Ok(0)}
            }
        }
    };

    impl PlainText | "PlainText_v1" | "PT_v1" | "Plain_v1" : {
        fn is_compatible(&self, _context: &impl Context, page: &ProcessedData<'_>)-> bool {
            page.0.page_type == PageType::PlainText
        }

        async fn extract_links(&self, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) {
            match &page.1 {
                DecodedData::InMemory{ result, .. } => {
                    let mut finder = linkify::LinkFinder::new();
                    finder.kinds(&[linkify::LinkKind::Url]);

                    let mut ct = 0usize;
                    for entry in finder.links(result.as_str()) {
                        match ExtractedLink::pack(&page.0.get_page().url, entry.as_str(), ExtractorMethodHint::new_without_meta(self.clone())) {
                            Ok(link) => {
                                if output.register_link(link) {
                                    ct += 1;
                                }
                            }
                            Err(error) => {
                                log::debug!("Was not able to parse {:?} from javascript. Error: {}", entry, error)
                            }
                        }
                    }
                    Ok(ct)
                }
                DecodedData::OffMemory{ .. } => {Err(())}
                DecodedData::None => {Ok(0)}
            }
        }
    };

    impl RawV1 | "RAW_v1" : {
        fn is_compatible(&self, _context: &impl Context, _page: &ProcessedData<'_>)-> bool {
            true
        }

        async fn extract_links(&self, _context: &impl Context, page: &ProcessedData<'_>, output: &mut ExtractorResult) {
            if let Some(in_memory) = page.0.data.content.as_in_memory() {
                let mut ct = 0usize;
                for entry in extract_possible_urls(in_memory.as_slice()) {
                    if let Some(encoding) = page.1.encoding() {
                        let encoded = &encoding.decode(entry).0;
                        match ExtractedLink::pack(
                            &page.0.get_page().url,
                            &encoded,
                            ExtractorMethodHint::new_without_meta(self.clone())
                        ) {
                            Ok(link) => {
                                if output.register_link(link) {
                                    ct += 1;
                                }
                                continue
                            }
                            Err(error) => {
                                log::debug!("Was not able to parse {:?} from javascript. Error: {}", entry, error)
                            }
                        }
                    }
                    let encoded = String::from_utf8_lossy(entry);
                    match ExtractedLink::pack(
                        &page.0.get_page().url,
                        &encoded,
                        ExtractorMethodHint::new_without_meta(self.clone())
                    ) {
                        Ok(link) => {
                            if output.register_link(link) {
                                ct += 1;
                            }
                        }
                        Err(error) => {
                            log::debug!("Was not able to parse {:?} from javascript. Error: {}", entry, error)
                        }
                    }

                }
                Ok(ct)
            } else {
                Ok(0)
            }

        }
    };
}

