//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::collections::{HashMap, HashSet};
use file_format::FileFormat;
use std::env;
use std::path::Path;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use tinyjson::JsonValue;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    generate_hashmaps();
    generate_stop_word_lists();
}


fn convert_to_codegen_map(m: HashMap<&'static str, HashSet<&str>>) -> (&'static str, phf_codegen::Map<&'static str>) {
    let mut new = phf_codegen::Map::<&'static str>::new();

    if m.values().any(|value| value.len() > 1) {
        for (k, v) in m.into_iter() {
            let mut targets: Vec<&str> = v.into_iter().collect();
            targets.sort();
            new.entry(k, format!("&[file_format::{}]", targets.join(", file_format::")).as_str());
        }
        ("phf::Map<&'static str, &'static [file_format::FileFormat]>", new)
    } else {
        for (k, v) in m.into_iter() {
            let targets: Vec<&str> = v.into_iter().collect();
            new.entry(k, format!("file_format::{}", targets.get(0).unwrap()).as_str());
        }
        ("phf::Map<&'static str, file_format::FileFormat>", new)
    }
}

/// 437 formats
macro_rules! register_in_map {
    (out: $out: ident, target: $targ: expr; $($format: expr,)+) => {
        let mut containers: std::collections::HashMap<&'static str, HashSet<&str>> = std::collections::HashMap::new();
        $(
        containers.entry($targ(&$format)).or_default().insert(stringify!($format));
        )+;
        $out = convert_to_codegen_map(containers);
    };
    (out: $out: ident, target: $targ: expr) => {
        register_in_map! {
            out: $out,
            target: $targ;
            FileFormat::Abiword,
            FileFormat::AbiwordTemplate,
            FileFormat::Ace,
            FileFormat::ActionsMediaVideo,
            FileFormat::Activemime,
            FileFormat::AdaptableScalableTextureCompression,
            FileFormat::AdaptiveMultiRate,
            FileFormat::AdditiveManufacturingFormat,
            FileFormat::AdobeIllustratorArtwork,
            FileFormat::AdobeIndesignDocument,
            FileFormat::AdobeIntegratedRuntime,
            FileFormat::AdobePhotoshopDocument,
            FileFormat::AdvancedAudioCoding,
            FileFormat::AdvancedStreamRedirector,
            FileFormat::AdvancedSystemsFormat,
            FileFormat::Alz,
            FileFormat::AmigaDiskFile,
            FileFormat::AndroidAppBundle,
            FileFormat::AndroidBinaryXml,
            FileFormat::AndroidPackage,
            FileFormat::AndroidResourceStorageContainer,
            FileFormat::AnimatedPortableNetworkGraphics,
            FileFormat::ApacheArrowColumnar,
            FileFormat::ApacheAvro,
            FileFormat::ApacheParquet,
            FileFormat::Appimage,
            FileFormat::AppleDiskImage,
            FileFormat::AppleIconImage,
            FileFormat::AppleItunesAudio,
            FileFormat::AppleItunesAudiobook,
            FileFormat::AppleItunesProtectedAudio,
            FileFormat::AppleItunesVideo,
            FileFormat::AppleQuicktime,
            FileFormat::ArbitraryBinaryData,
            FileFormat::ArchivedByRobertJung,
            FileFormat::Atari7800Rom,
            FileFormat::Atom,
            FileFormat::Au,
            FileFormat::AudioCodec3,
            FileFormat::AudioInterchangeFileFormat,
            FileFormat::AudioVideoInterleave,
            FileFormat::AudioVisualResearch,
            FileFormat::AutocadDrawing,
            FileFormat::Autodesk123d,
            FileFormat::AutodeskAlias,
            FileFormat::AutodeskAnimator,
            FileFormat::AutodeskAnimatorPro,
            FileFormat::AutodeskInventorAssembly,
            FileFormat::AutodeskInventorDrawing,
            FileFormat::AutodeskInventorPart,
            FileFormat::AutodeskInventorPresentation,
            FileFormat::Av1ImageFileFormat,
            FileFormat::Av1ImageFileFormatSequence,
            FileFormat::BdavMpeg2TransportStream,
            FileFormat::BetterPortableGraphics,
            FileFormat::Bittorrent,
            FileFormat::Blender,
            FileFormat::BmfontAscii,
            FileFormat::BmfontBinary,
            FileFormat::BroadBandEbook,
            FileFormat::Bzip,
            FileFormat::Bzip2,
            FileFormat::Bzip3,
            FileFormat::Cabinet,
            FileFormat::CanonRaw,
            FileFormat::CanonRaw2,
            FileFormat::CanonRaw3,
            FileFormat::CdAudio,
            FileFormat::Cinema4d,
            FileFormat::Cineon,
            FileFormat::CircuitDiagramDocument,
            FileFormat::ClojureScript,
            FileFormat::CollaborativeDesignActivity,
            FileFormat::Commodore64Cartridge,
            FileFormat::Commodore64Program,
            FileFormat::CommonObjectFileFormat,
            FileFormat::CompoundFileBinary,
            FileFormat::CorelPresentations,
            FileFormat::CorelPresentations7,
            FileFormat::Cpio,
            FileFormat::CreativeVoice,
            FileFormat::DalvikExecutable,
            FileFormat::DebianPackage,
            FileFormat::DerCertificate,
            FileFormat::DesignWebFormat,
            FileFormat::DesignWebFormatXps,
            FileFormat::DigitalImagingAndCommunicationsInMedicine,
            FileFormat::DigitalPictureExchange,
            FileFormat::Djvu,
            FileFormat::DrawingExchangeFormatAscii,
            FileFormat::DrawingExchangeFormatBinary,
            FileFormat::Drawio,
            FileFormat::DynamicLinkLibrary,
            FileFormat::EightBitSampledVoice,
            FileFormat::ElectronicPublication,
            FileFormat::EmbeddedOpentype,
            FileFormat::Empty,
            FileFormat::EncapsulatedPostscript,
            FileFormat::EnterpriseApplicationArchive,
            FileFormat::ExecutableAndLinkableFormat,
            FileFormat::ExperimentalComputingFacility,
            FileFormat::Extensible3d,
            FileFormat::ExtensibleArchive,
            FileFormat::ExtensibleBinaryMetaLanguage,
            FileFormat::ExtensibleMarkupLanguage,
            FileFormat::ExtensibleStylesheetLanguageTransformations,
            FileFormat::Farbfeld,
            FileFormat::Fasttracker2ExtendedModule,
            FileFormat::Fictionbook,
            FileFormat::FictionbookZip,
            FileFormat::Filmbox,
            FileFormat::FlashCs5Project,
            FileFormat::FlashMp4Audio,
            FileFormat::FlashMp4Audiobook,
            FileFormat::FlashMp4ProtectedVideo,
            FileFormat::FlashMp4Video,
            FileFormat::FlashProject,
            FileFormat::FlashVideo,
            FileFormat::FlexibleAndInteroperableDataTransfer,
            FileFormat::FlexibleImageTransportSystem,
            FileFormat::FreeLosslessAudioCodec,
            FileFormat::FreeLosslessImageFormat,
            FileFormat::FujifilmRaw,
            FileFormat::Fusion360,
            FileFormat::GameBoyAdvanceRom,
            FileFormat::GameBoyColorRom,
            FileFormat::GameBoyRom,
            FileFormat::GameGearRom,
            FileFormat::GeographyMarkupLanguage,
            FileFormat::GettextMachineObject,
            FileFormat::GlTransmissionFormatBinary,
            FileFormat::GoogleChromeExtension,
            FileFormat::GoogleDraco,
            FileFormat::GpsExchangeFormat,
            FileFormat::GraphicsInterchangeFormat,
            FileFormat::Gzip,
            FileFormat::HighEfficiencyImageCoding,
            FileFormat::HighEfficiencyImageCodingSequence,
            FileFormat::HighEfficiencyImageFileFormat,
            FileFormat::HighEfficiencyImageFileFormatSequence,
            FileFormat::HypertextMarkupLanguage,
            FileFormat::Icalendar,
            FileFormat::IccProfile,
            FileFormat::ImpulseTrackerModule,
            FileFormat::IndesignMarkupLanguage,
            FileFormat::InitialGraphicsExchangeSpecification,
            FileFormat::InterQuakeExport,
            FileFormat::InterQuakeModel,
            FileFormat::IosAppStorePackage,
            FileFormat::Iso9660,
            FileFormat::JavaArchive,
            FileFormat::JavaClass,
            FileFormat::JavaKeystore,
            FileFormat::JointPhotographicExpertsGroup,
            FileFormat::Jpeg2000Codestream,
            FileFormat::Jpeg2000Part1,
            FileFormat::Jpeg2000Part2,
            FileFormat::Jpeg2000Part3,
            FileFormat::Jpeg2000Part6,
            FileFormat::JpegExtendedRange,
            FileFormat::JpegLs,
            FileFormat::JpegNetworkGraphics,
            FileFormat::JpegXl,
            FileFormat::JsonFeed,
            FileFormat::KeyholeMarkupLanguage,
            FileFormat::KeyholeMarkupLanguageZip,
            FileFormat::KhronosTexture,
            FileFormat::KhronosTexture2,
            FileFormat::Larc,
            FileFormat::Latex,
            FileFormat::LempelZivFiniteStateEntropy,
            FileFormat::LempelZivMarkovChainAlgorithm,
            FileFormat::Lha,
            FileFormat::LinearExecutable,
            FileFormat::LlvmBitcode,
            FileFormat::LongRangeZip,
            FileFormat::LuaBytecode,
            FileFormat::LuaScript,
            FileFormat::Lz4,
            FileFormat::Lzip,
            FileFormat::Lzop,
            FileFormat::MachO,
            FileFormat::MacosAlias,
            FileFormat::Magicavoxel,
            FileFormat::MagickImageFileFormat,
            FileFormat::MaterialExchangeFormat,
            FileFormat::MathematicalMarkupLanguage,
            FileFormat::Matroska3dVideo,
            FileFormat::MatroskaAudio,
            FileFormat::MatroskaSubtitles,
            FileFormat::MatroskaVideo,
            FileFormat::MayaAscii,
            FileFormat::MayaBinary,
            FileFormat::MegaDriveRom,
            FileFormat::MetaInformationEncapsulation,
            FileFormat::MicrosoftAccess2007Database,
            FileFormat::MicrosoftAccessDatabase,
            FileFormat::MicrosoftCompiledHtmlHelp,
            FileFormat::MicrosoftDigitalVideoRecording,
            FileFormat::MicrosoftDirectdrawSurface,
            FileFormat::MicrosoftExcelSpreadsheet,
            FileFormat::MicrosoftPowerpointPresentation,
            FileFormat::MicrosoftProjectPlan,
            FileFormat::MicrosoftPublisherDocument,
            FileFormat::MicrosoftReader,
            FileFormat::MicrosoftSoftwareInstaller,
            FileFormat::MicrosoftVirtualHardDisk,
            FileFormat::MicrosoftVirtualHardDisk2,
            FileFormat::MicrosoftVisioDrawing,
            FileFormat::MicrosoftVisualStudioExtension,
            FileFormat::MicrosoftVisualStudioSolution,
            FileFormat::MicrosoftWordDocument,
            FileFormat::MicrosoftWorks6Spreadsheet,
            FileFormat::MicrosoftWorksDatabase,
            FileFormat::MicrosoftWorksSpreadsheet,
            FileFormat::MicrosoftWorksWordProcessor,
            FileFormat::MicrosoftWrite,
            FileFormat::Mobipocket,
            FileFormat::Model3dAscii,
            FileFormat::Model3dBinary,
            FileFormat::MonkeysAudio,
            FileFormat::MozillaArchive,
            FileFormat::Mp3Url,
            FileFormat::Mpeg12AudioLayer2,
            FileFormat::Mpeg12AudioLayer3,
            FileFormat::Mpeg12Video,
            FileFormat::Mpeg2TransportStream,
            FileFormat::Mpeg4Part14,
            FileFormat::Mpeg4Part14Audio,
            FileFormat::Mpeg4Part14Subtitles,
            FileFormat::Mpeg4Part14Video,
            FileFormat::MpegDashMpd,
            FileFormat::MsDosBatch,
            FileFormat::MsDosExecutable,
            FileFormat::Mtv,
            FileFormat::MultiLayerArchive,
            FileFormat::MultipleImageNetworkGraphics,
            FileFormat::Musepack,
            FileFormat::MusicalInstrumentDigitalInterface,
            FileFormat::Musicxml,
            FileFormat::MusicxmlZip,
            FileFormat::NeoGeoPocketColorRom,
            FileFormat::NeoGeoPocketRom,
            FileFormat::NewExecutable,
            FileFormat::NikonElectronicFile,
            FileFormat::Nintendo64Rom,
            FileFormat::NintendoDsRom,
            FileFormat::NintendoEntertainmentSystemRom,
            FileFormat::NintendoSwitchExecutable,
            FileFormat::NintendoSwitchPackage,
            FileFormat::NintendoSwitchRom,
            FileFormat::OfficeOpenXmlDocument,
            FileFormat::OfficeOpenXmlDrawing,
            FileFormat::OfficeOpenXmlPresentation,
            FileFormat::OfficeOpenXmlSpreadsheet,
            FileFormat::OggFlac,
            FileFormat::OggMedia,
            FileFormat::OggMultiplexedMedia,
            FileFormat::OggOpus,
            FileFormat::OggSpeex,
            FileFormat::OggTheora,
            FileFormat::OggVorbis,
            FileFormat::OlympusRawFormat,
            FileFormat::OpendocumentDatabase,
            FileFormat::OpendocumentFormula,
            FileFormat::OpendocumentFormulaTemplate,
            FileFormat::OpendocumentGraphics,
            FileFormat::OpendocumentGraphicsTemplate,
            FileFormat::OpendocumentPresentation,
            FileFormat::OpendocumentPresentationTemplate,
            FileFormat::OpendocumentSpreadsheet,
            FileFormat::OpendocumentSpreadsheetTemplate,
            FileFormat::OpendocumentText,
            FileFormat::OpendocumentTextMaster,
            FileFormat::OpendocumentTextMasterTemplate,
            FileFormat::OpendocumentTextTemplate,
            FileFormat::Openexr,
            FileFormat::Opennurbs,
            FileFormat::Openraster,
            FileFormat::Opentype,
            FileFormat::Openxps,
            FileFormat::OptimizedDalvikExecutable,
            FileFormat::PanasonicRaw,
            FileFormat::PcapDump,
            FileFormat::PcapNextGenerationDump,
            FileFormat::PemCertificate,
            FileFormat::PemCertificateSigningRequest,
            FileFormat::PemPrivateKey,
            FileFormat::PemPublicKey,
            FileFormat::PerlScript,
            FileFormat::PersonalStorageTable,
            FileFormat::PgpMessage,
            FileFormat::PgpPrivateKeyBlock,
            FileFormat::PgpPublicKeyBlock,
            FileFormat::PgpSignature,
            FileFormat::PgpSignedMessage,
            FileFormat::PictureExchange,
            FileFormat::PlainText,
            FileFormat::Pmarc,
            FileFormat::PolygonAscii,
            FileFormat::PolygonBinary,
            FileFormat::PortableArbitraryMap,
            FileFormat::PortableBitmap,
            FileFormat::PortableDocumentFormat,
            FileFormat::PortableExecutable,
            FileFormat::PortableFloatmap,
            FileFormat::PortableGraymap,
            FileFormat::PortableNetworkGraphics,
            FileFormat::PortablePixmap,
            FileFormat::Postscript,
            FileFormat::PythonScript,
            FileFormat::QemuCopyOnWrite,
            FileFormat::QualcommPurevoice,
            FileFormat::QuiteOkAudio,
            FileFormat::QuiteOkImage,
            FileFormat::RadianceHdr,
            FileFormat::Realaudio,
            FileFormat::ReallySimpleSyndication,
            FileFormat::Realmedia,
            FileFormat::Realvideo,
            FileFormat::RedHatPackageManager,
            FileFormat::RichTextFormat,
            FileFormat::RoshalArchive,
            FileFormat::RubyScript,
            FileFormat::Rzip,
            FileFormat::ScalableVectorGraphics,
            FileFormat::ScreamTracker3Module,
            FileFormat::SegaMasterSystemRom,
            FileFormat::Seqbox,
            FileFormat::SevenZip,
            FileFormat::Shapefile,
            FileFormat::ShellScript,
            FileFormat::ShoutcastPlaylist,
            FileFormat::SiliconGraphicsImage,
            FileFormat::SiliconGraphicsMovie,
            FileFormat::SimpleObjectAccessProtocol,
            FileFormat::Sketchup,
            FileFormat::SmallWebFormat,
            FileFormat::Snappy,
            FileFormat::SolidworksAssembly,
            FileFormat::SolidworksDrawing,
            FileFormat::SolidworksPart,
            FileFormat::SonyDsdStreamFile,
            FileFormat::SonyMovie,
            FileFormat::Soundfont2,
            FileFormat::SpaceclaimDocument,
            FileFormat::Sqlite3,
            FileFormat::Squashfs,
            FileFormat::StandardForTheExchangeOfProductModelData,
            FileFormat::Starcalc,
            FileFormat::Starchart,
            FileFormat::Stardraw,
            FileFormat::Starimpress,
            FileFormat::Starmath,
            FileFormat::Starwriter,
            FileFormat::StereolithographyAscii,
            FileFormat::Stuffit,
            FileFormat::StuffitX,
            FileFormat::SubripText,
            FileFormat::SunXmlCalc,
            FileFormat::SunXmlCalcTemplate,
            FileFormat::SunXmlDraw,
            FileFormat::SunXmlDrawTemplate,
            FileFormat::SunXmlImpress,
            FileFormat::SunXmlImpressTemplate,
            FileFormat::SunXmlMath,
            FileFormat::SunXmlWriter,
            FileFormat::SunXmlWriterGlobal,
            FileFormat::SunXmlWriterTemplate,
            FileFormat::TagImageFileFormat,
            FileFormat::TapeArchive,
            FileFormat::Tasty,
            FileFormat::ThirdGenerationPartnershipProject,
            FileFormat::ThirdGenerationPartnershipProject2,
            FileFormat::ThreeDimensionalManufacturingFormat,
            FileFormat::ThreeDimensionalStudio,
            FileFormat::ThreeDimensionalStudioMax,
            FileFormat::TiledMapXml,
            FileFormat::TiledTilesetXml,
            FileFormat::TimedTextMarkupLanguage,
            FileFormat::ToolCommandLanguageScript,
            FileFormat::TrainingCenterXml,
            FileFormat::Truetype,
            FileFormat::UltimateSoundtrackerModule,
            FileFormat::UniformOfficeFormatPresentation,
            FileFormat::UniformOfficeFormatSpreadsheet,
            FileFormat::UniformOfficeFormatText,
            FileFormat::Universal3d,
            FileFormat::UniversalSceneDescriptionAscii,
            FileFormat::UniversalSceneDescriptionBinary,
            FileFormat::UniversalSceneDescriptionZip,
            FileFormat::UniversalSubtitleFormat,
            FileFormat::UnixArchiver,
            FileFormat::UnixCompress,
            FileFormat::Vcalendar,
            FileFormat::Vcard,
            FileFormat::VirtualMachineDisk,
            FileFormat::VirtualRealityModelingLanguage,
            FileFormat::VirtualboxVirtualDiskImage,
            FileFormat::WaveformAudio,
            FileFormat::Wavpack,
            FileFormat::WebApplicationArchive,
            FileFormat::WebOpenFontFormat,
            FileFormat::WebOpenFontFormat2,
            FileFormat::WebVideoTextTracks,
            FileFormat::WebassemblyBinary,
            FileFormat::WebassemblyText,
            FileFormat::Webm,
            FileFormat::Webp,
            FileFormat::WindowsAnimatedCursor,
            FileFormat::WindowsAppBundle,
            FileFormat::WindowsAppPackage,
            FileFormat::WindowsBitmap,
            FileFormat::WindowsCursor,
            FileFormat::WindowsIcon,
            FileFormat::WindowsImagingFormat,
            FileFormat::WindowsMediaAudio,
            FileFormat::WindowsMediaPlaylist,
            FileFormat::WindowsMediaVideo,
            FileFormat::WindowsMetafile,
            FileFormat::WindowsRecordedTvShow,
            FileFormat::WindowsShortcut,
            FileFormat::WordperfectDocument,
            FileFormat::WordperfectGraphics,
            FileFormat::WordperfectMacro,
            FileFormat::WordperfectPresentations,
            FileFormat::XPixmap,
            FileFormat::Xap,
            FileFormat::Xbox360Executable,
            FileFormat::XboxExecutable,
            FileFormat::XmlLocalizationInterchangeFileFormat,
            FileFormat::XmlShareablePlaylistFormat,
            FileFormat::Xpinstall,
            FileFormat::Xz,
            FileFormat::Zip,
            FileFormat::Zoo,
            FileFormat::Zpaq,
            FileFormat::Zstandard,
        }
    }
}

#[allow(redundant_semicolons)]
fn generate_hashmaps(){
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen_file_format.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let media;
    register_in_map!(
        out: media,
        target: FileFormat::media_type
    );

    writeln!(
        &mut file,
        "static MEDIA_TYPE_TO_FILE_FORMAT: {} = \n{};\n",
        media.0,
        media.1.build()
    ).unwrap();

    let format;

    register_in_map!(
        out: format,
        target: FileFormat::extension
    );

    writeln!(
        &mut file,
        "static EXTENSION_FILE_FORMAT: {} = \n{};\n",
        format.0,
        format.1.build()
    ).unwrap();

}


fn generate_stop_word_lists() {
    let mut data = File::options().read(true).open("./external/stopwords/iso/stopwords-iso.json").unwrap();
    let mut content = String::new();
    data.read_to_string(&mut content).unwrap();
    drop(data);
    let parsed: JsonValue = content.parse().unwrap();
    let object: &HashMap<_, _> = parsed.get().unwrap();
    build_stop_word_library(object);
}



fn build_stop_word_library(object: &HashMap<String, JsonValue>) {
    let base = Path::new("./data/stopwords/iso");
    if !base.exists() {
        std::fs::create_dir_all(base).unwrap();
    }
    println!("crate: {}", std::fs::canonicalize(base).unwrap().to_str().unwrap());
    for (k, v) in object.iter() {
        let lang = isolang::Language::from_639_1(k.as_str()).expect(format!("Why is {k} not an iso language?").as_str());
        let values: Vec<_> = v.get::<Vec<_>>().unwrap().iter().map(|value| value.get::<String>().unwrap().to_string()).collect();
        let mut file = BufWriter::new(File::options().write(true).truncate(true).create(true).open(base.join(format!("{}.txt", lang.to_639_1().unwrap()))).unwrap());
        for v in values {
            writeln!(&mut file, "{v}").unwrap();
        }
    }
}