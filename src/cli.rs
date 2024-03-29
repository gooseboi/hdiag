use clap::{Parser, ValueEnum};
use std::{fs, io, path::PathBuf};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Input file to read
    #[arg(short)]
    input_file: PathBuf,

    /// Type of the input file
    #[arg(short = 't', value_enum, default_value_t = FileTypes::Inferred)]
    input_type: FileTypes,

    /// Path of the output file.
    /// Default is filename with svg extension
    #[arg(short = 'o')]
    output_path: Option<PathBuf>,

    /// What type of svg should be outputted
    #[arg(short = 'f', value_enum, default_value_t = FontFormats::Path)]
    font_output_format: FontFormats,

    /// What theme should the svg be exported in
    #[arg(long = "theme", value_enum, default_value_t = OutputTheme::Dark)]
    output_theme: OutputTheme,

    /// Should the export have a background
    #[arg(short = 'b', long = "background")]
    output_background: bool,

    /// Should the export bundle the program's source
    #[arg(long = "source")]
    embed_source: bool,

    /// What scale should the export be in
    #[arg(short, long = "scale", default_value_t = 1)]
    scale: u8,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FileTypes {
    /// Excalidraw JSON file (https://excalidraw.com)
    Excalidraw,
    /// Drawio mxGraph file (https://drawio.com)
    Drawio,
    /// Inferred from the file extension or content.
    /// The extension is preferred
    Inferred,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FontFormats {
    /// The raw svg outputted by the renderer for the filetype
    Raw,
    /// Embed no fonts into the svg, and rely on the system installed ones
    NoFont,
    /// Embed the font files inside the svg
    Embed,
    /// Convert all text in the svg to paths.
    /// This means no embedded fonts, and no text in the svg, but less
    /// filesize and more portable
    Path,
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum OutputTheme {
    Dark,
    Light,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileType {
    Excalidraw,
    Drawio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontFormat {
    Raw,
    Embed,
    Path,
    NoFont,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOpts {
    pub theme: OutputTheme,
    pub include_background: bool,
    pub embed_source: bool,
    pub scale: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opts {
    pub input_file: PathBuf,
    pub input_type: FileType,
    pub output_file: PathBuf,
    pub output_format: FontFormat,
    pub export: ExportOpts,
}

fn is_valid_json<R: io::Read>(r: R) -> bool {
    serde_json::from_reader::<R, ()>(r).is_ok()
}

fn is_valid_xml<R: io::Read>(r: R) -> bool {
    let mut reader = quick_xml::reader::Reader::from_reader(io::BufReader::new(r));
    let mut buf = vec![];
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Eof) => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
}

impl Opts {
    pub fn parse() -> Self {
        let cli = Cli::parse();

        let input_file = cli.input_file;

        let output_path = cli.output_path.map_or_else(
            || {
                let name = input_file
                    .file_name()
                    .expect("File had no file name to infert output file");
                let mut p = PathBuf::from(name);
                p.set_extension("svg");
                p
            },
            |o| o,
        );

        let input_type = match cli.input_type {
            FileTypes::Excalidraw => FileType::Excalidraw,
            FileTypes::Drawio => FileType::Drawio,
            FileTypes::Inferred => {
                match input_file
                    .extension()
                    .map(|s| s.to_str().expect("Extension was not UTF-8"))
                {
                    Some("excalidraw") => FileType::Excalidraw,
                    Some("drawio") => FileType::Drawio,
                    Some(_) | None => {
                        let f = fs::OpenOptions::new()
                            .read(true)
                            .write(false)
                            .open(&input_file)
                            .expect("Could not open file to infer file type");
                        if is_valid_json(&f) {
                            FileType::Excalidraw
                        } else if is_valid_xml(&f) {
                            FileType::Drawio
                        } else {
                            panic!("Could not infer filetype for {}", input_file.display());
                        }
                    }
                }
            }
        };

        let output_format = match cli.font_output_format {
            FontFormats::Raw => FontFormat::Raw,
            FontFormats::Embed => FontFormat::Embed,
            FontFormats::Path => FontFormat::Path,
            FontFormats::NoFont => FontFormat::NoFont,
        };

        let export_opts = ExportOpts {
            theme: cli.output_theme,
            include_background: cli.output_background,
            embed_source: cli.embed_source,
            scale: cli.scale,
        };

        Self {
            input_file,
            input_type,
            output_file: output_path,
            output_format,
            export: export_opts,
        }
    }
}
