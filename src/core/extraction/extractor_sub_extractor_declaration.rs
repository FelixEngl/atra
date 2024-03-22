/// A macro to declare sub extractors
#[macro_export]
macro_rules! declare_sub_extractor {
    ($($name: ident $(| $alt:literal)* => $action: ident;)+) => {
        #[derive(Debug, Eq, PartialEq, Hash, Serialize, Deserialize, Copy, Clone)]
        pub enum SubExtractor {
            $(
                $(
                    #[serde(alias = $alt)]
                )*
                $name,
            )+
        }

        impl SubExtractor {
            pub const COUNT:usize = 0$(+
            ((match SubExtractor::$name{ _ => 1 }) as usize))+;

            pub const ALL_ENTRIES: [SubExtractor; {SubExtractor::COUNT}] = [
                $(
                SubExtractor::$name,
                )+
            ];
        }

        impl ExtractorMetaFactory for SubExtractor {
            fn create_meta(&self, meta: SubExtractorMeta) -> ExtractorMeta {
                ExtractorMeta {
                    extractor: self.clone(),
                    meta
                }
            }

            fn create_empty_meta(&self) -> ExtractorMeta {
                ExtractorMeta {
                    extractor: self.clone(),
                    meta: SubExtractorMeta::None
                }
            }
        }

        impl SubExtractor {
            /// Extracts the urls with the sub extractor.
            pub async fn extract(&self, page: &ProcessedPage<'_>, context: &impl Context, extracted_domains_count: usize) -> Option<HashSet<ExtractedLink>> {
                match self {
                    $(
                        SubExtractor::$name => {
                            $action::decode(page, context, self, extracted_domains_count).await
                        }
                    )+
                }
            }
        }
    };
}

