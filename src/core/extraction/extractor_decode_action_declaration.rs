/// A macro to define an extractor action.
#[macro_export]
macro_rules! define_decode_action {
    (
        $v: vis struct $name: ident: fallback;
        fn decoded($page1: ident, $context1: ident, $factory1: ident, $content: ident) -> $expr1: block
        fn decoded_file($page2: ident, $context2: ident, $factory2: ident, $path_to_file: ident) -> $expr2: block
        fn not_decoded($page3: ident, $context3: ident) -> $expr3: block
    ) => {
        $v struct $name;

        impl DecodeAction for $name {
            async fn decode(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, extracted_domains_count: usize) -> Option<HashSet<ExtractedLink>> {
                if extracted_domains_count != 0 {
                    return None
                }
                match &page.1 {
                    DecodedData::InMemory {result, ..} => {
                        Self::decoded_small(page, context, factory, result).await
                    }
                    DecodedData::OffMemory {result, ..} => {
                        Self::decoded_big(page, context, factory, result).await
                    }
                    DecodedData::None => {
                        Self::not_decoded(page, context).await
                    }
                }
            }

            async fn decoded_small($page1: &ProcessedPage<'_>, $context1: &impl Context, $factory1: &impl ExtractorMetaFactory, $content: &String) -> Option<HashSet<ExtractedLink>> $expr1

            async fn decoded_big($page2: &ProcessedPage<'_>, $context2: &impl Context, $factory2: &impl ExtractorMetaFactory, $path_to_file: &DecodedDataFilePathBuf) -> Option<HashSet<ExtractedLink>> $expr2

            async fn not_decoded($page3: &ProcessedPage<'_>, $context3: &impl Context) -> Option<HashSet<ExtractedLink>> $expr3
        }
    };
    (
        $v: vis struct $name: ident: all;
        fn decoded($page1: ident, $context1: ident, $factory1: ident, $content: ident) -> $expr1: block
        fn decoded_file($page2: ident, $context2: ident, $factory2: ident, $path_to_file: ident) -> $expr2: block
        fn not_decoded($page3: ident, $context3: ident) -> $expr3: block
    ) => {
        $v struct $name;

        impl DecodeAction for $name {
            async fn decode(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, _: usize) -> Option<HashSet<ExtractedLink>> {
                match &page.1 {
                    DecodedData::InMemory {result, ..} => {
                        Self::decoded_small(page, context, factory, result).await
                    }
                    DecodedData::OffMemory {result, ..} => {
                        Self::decoded_big(page, context, factory, result).await
                    }
                    DecodedData::None => {
                        Self::not_decoded(page, context).await
                    }
                }
            }

            async fn decoded_small($page1: &ProcessedPage<'_>, $context1: &impl Context, $factory1: &impl ExtractorMetaFactory, $content: &String) -> Option<HashSet<ExtractedLink>> $expr1

            async fn decoded_big($page2: &ProcessedPage<'_>, $context2: &impl Context, $factory2: &impl ExtractorMetaFactory, $path_to_file: &DecodedDataFilePathBuf) -> Option<HashSet<ExtractedLink>> $expr2

            async fn not_decoded($page3: &ProcessedPage<'_>, $context3: &impl Context) -> Option<HashSet<ExtractedLink>> $expr3
        }
    };
    (
        $v: vis struct $name: ident: $($target: ident),+;
        fn decoded($page1: ident, $context1: ident, $factory1: ident, $content: ident) -> $expr1: block
        fn decoded_file($page2: ident, $context2: ident, $factory2: ident, $path_to_file: ident) -> $expr2: block
        fn not_decoded($page3: ident, $context3: ident) -> $expr3: block
    ) => {
        $v struct $name;

        impl DecodeAction for $name {
            async fn decode(page: &ProcessedPage<'_>, context: &impl Context, factory: &impl ExtractorMetaFactory, _: usize) -> Option<HashSet<ExtractedLink>> {
                match &page.0.page_type {
                    $(
                        PageType::$target => {
                            match &page.1 {
                                DecodedData::InMemory {result, ..} => {
                                    Self::decoded_small(page, context, factory, result).await
                                }
                                DecodedData::OffMemory {result, ..} => {
                                    Self::decoded_big(page, context, factory, result).await
                                }
                                DecodedData::None => {
                                    Self::not_decoded(page, context).await
                                }
                            }
                        }
                    )+
                    _ => None
                }
            }

            async fn decoded_small($page1: &ProcessedPage<'_>, $context1: &impl Context, $factory1: &impl ExtractorMetaFactory, $content: &String) -> Option<HashSet<ExtractedLink>> $expr1

            async fn decoded_big($page2: &ProcessedPage<'_>, $context2: &impl Context, $factory2: &impl ExtractorMetaFactory, $path_to_file: &DecodedDataFilePathBuf) -> Option<HashSet<ExtractedLink>> $expr2

            async fn not_decoded($page3: &ProcessedPage<'_>, $context3: &impl Context) -> Option<HashSet<ExtractedLink>> $expr3
        }
    };

    (
        $v: vis struct $name: ident: $tt:tt;
        fn decoded($page1: ident, $context1: ident, $factory1: ident, $content: ident) -> $expr1: block
        fn decoded_file($page2: ident, $context2: ident, $factory2: ident, $path_to_file: ident) -> $expr2: block
    ) => {
        define_decode_action!(
            $v struct $name: $tt;
            fn decoded($page1, $context1, $factory1, $content) -> $expr1
            fn decoded_file($page2, $context2, $factory2, $path_to_file) -> $expr2
            fn not_decoded(_page, _context)-> {None}
        );
    };

    (
        $v: vis struct $name: ident: $tt:tt;
        fn not_decoded($page3: ident, $context3: ident) -> $expr3: block
    ) => {
        define_decode_action!(
            $v struct $name: $tt;
            fn decoded(_page, _context, _factory, _content) -> {None}
            fn decoded_file(_page, _context, _factory, _path_to_file) -> {None}
            fn not_decoded($page3, $context3)-> $expr3
        );
    };
}

