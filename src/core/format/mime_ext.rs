use std::sync::LazyLock;
use mime::{Mime};
use paste::paste;
use const_format::concatcp;

macro_rules! mime_declarations {

    ($($name: ident: $typ: literal / $subtype: literal $(+ $suffix: literal)?),+ $(,)?) => {
        paste! {
            $(
                const [<$name _RAW_TYPE>]: &str = $typ;
                const [<$name _RAW_SUBTYPE>]: &str = $subtype;
                $(const [<$name _RAW_SUFFIX>]: &str = $suffix;)?
                const [<$name _RAW>]: &str = concatcp!($typ, "/", $subtype $(, "+", $suffix)?);
                pub static $name: LazyLock<Mime> = LazyLock::new(||  [<$name _RAW>] .parse::<Mime>().unwrap());
            )+

            #[cfg(test)]
            mod test {
                use mime::{Mime};
                $(
                    #[test]
                    fn [<test_ $name>]() {
                         super::[<$name _RAW>].parse::<Mime>().expect("Can not parse the value!");
                    }
                )+
            }
        }
    };
}

mime_declarations! {
    APPLICATION_XML: "application" / "xml",
    APPLICATION_RTF: "application" / "rtf",
    APPLICATION_OOXML_STAR: "application" / "vnd.openxmlformats-officedocument.wordprocessingml.*",
    APPLICATION_OOXML_DOCX: "application" / "vnd.openxmlformats-officedocument.wordprocessingml.document",
    APPLICATION_OOXML_XLSX: "application" / "vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    APPLICATION_OOXML_PPTX: "application" / "vnd.openxmlformats-officedocument.presentationml.presentation",
    APPLICATION_ODF_STAR: "application" / "vnd.oasis.opendocument.*",
    APPLICATION_ODF_TEXT: "application" / "vnd.oasis.opendocument.text",
    APPLICATION_ODF_SPREADSHEET: "application" / "vnd.oasis.opendocument.spreadsheet",
    APPLICATION_ODF_PRESENTATION: "application" / "vnd.oasis.opendocument.presentation",
    APPLICATION_ODF_GRAPHICS: "application" / "vnd.oasis.opendocument.graphics",
    APPLICATION_ODF_CHART: "application" / "vnd.oasis.opendocument.chart",
    APPLICATION_ODF_FORMULAR: "application" / "vnd.oasis.opendocument.formula",
    APPLICATION_ODF_IMAGE: "application" / "vnd.oasis.opendocument.image",
    APPLICATION_ODF_TEST_MASTER: "application" / "vnd.oasis.opendocument.text-master",
    APPLICATION_ODF_TEXT_TEMPLATE: "application" / "vnd.oasis.opendocument.text-template",
    APPLICATION_ODF_SPREADSHEET_TEMPLATE: "application" / "vnd.oasis.opendocument.spreadsheet-template",
    APPLICATION_ODF_PRESENTATION_TEMPLATE: "application" / "vnd.oasis.opendocument.presentation-template",
    APPLICATION_ODF_GRAPHICS_TEMPLATE: "application" / "vnd.oasis.opendocument.graphics-template",
    APPLICATION_ODF_CHART_TEMPLATE: "application" / "vnd.oasis.opendocument.chart-template",
    APPLICATION_ODF_FORMULAR_TEMPLATE: "application" / "vnd.oasis.opendocument.formula-template",
    APPLICATION_ODF_IMAGE_TEMPLATE: "application" / "vnd.oasis.opendocument.image-template",
    APPLICATION_ODF_TEXT_WEB: "application" / "vnd.oasis.opendocument.text-web",
    AUDIO_MP3_URL: "audio" / "x-mpegurl",
}