#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::{
    fs,
    io::{Cursor, Read, Write as _},
    path::{Path, PathBuf},
};

use base64::prelude::*;
use clap::{Parser, ValueEnum};
use color_eyre::{
    eyre::{bail, ContextCompat, WrapErr},
    Result,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use zip::ZipArchive;

mod excalidraw;
mod serve_zip;

const EXCALIDRAW_APP_ASSETS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-app.zip"));
const EXCALIDRAW_FONTS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-fonts.zip"));

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short)]
    input_file: PathBuf,

    #[arg(short = 't', value_enum, default_value_t = FileTypes::Inferred)]
    input_type: FileTypes,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FileTypes {
    Excalidraw,
    Drawio,
    Inferred,
}

fn get_used_fonts(style: &str) -> Result<Vec<(&str, &str)>> {
    let mut slice = &style[..];

    let pat = "excalidraw-assets/";
    let l = pat.len();
    let mut fonts = vec![];
    while let Some(idx) = slice.find(pat) {
        let start_idx = idx + l;
        let end_idx = slice[start_idx..]
            .find('"')
            .wrap_err_with(|| format!("Unexpected invalid style: {style}"))?;
        let end_idx = end_idx + start_idx;
        fonts.push(&slice[start_idx..end_idx]);
        slice = &slice[end_idx..];
    }

    let mut slice = &style[..];

    let pat = "font-family: \"";
    let l = pat.len();
    let mut font_names = vec![];
    while let Some(idx) = slice.find(pat) {
        let start_idx = idx + l;
        let end_idx = slice[start_idx..]
            .find('"')
            .wrap_err_with(|| format!("Unexpected invalid style: {style}"))?;
        let end_idx = end_idx + start_idx;
        font_names.push(&slice[start_idx..end_idx]);
        slice = &slice[end_idx..];
    }
    Ok(font_names.into_iter().zip(fonts.into_iter()).collect())
}

fn get_used_fonts_base64(style: &str) -> Result<String> {
    let fonts = get_used_fonts(style).wrap_err("Failed processing font files")?;
    let fonts: Result<Vec<(&str, String)>> = fonts
        .into_iter()
        .map(|(font_name, font_file)| {
            let mut zip = ZipArchive::new(Cursor::new(EXCALIDRAW_FONTS))
                .wrap_err("Failed to read zip archive as a zip archive")?;
            let bytes = match zip.by_name(font_file.as_ref()) {
                Ok(mut entry) => {
                    let mut bytes = vec![];
                    std::io::copy(&mut entry, &mut bytes)
                        .wrap_err_with(|| format!("Failed to write bytes to buffer"))?;
                    bytes
                }
                Err(e) => bail!("Failed to find font in zip archive: {e}"),
            };
            let mut buf = vec![];
            buf.resize(bytes.len() * 4 / 3 + 4, 0);

            let bytes_written = BASE64_STANDARD.encode_slice(&bytes, &mut buf).unwrap();

            // shorten our vec down to just what was written
            buf.truncate(bytes_written);
            let b64 = String::from_utf8(buf).wrap_err("Output of base64 should be UTF-8")?;
            Ok((font_name, b64))
        })
        .collect();
    let fonts = fonts.wrap_err("Failed to gget font file in base 64")?;
    let fonts_str = fonts.into_iter().map(|(font_name, font_b64)| format!("@font-face {{ font-family: \"{font_name}\"; src: url(data:font/woff2;charset=utf-8;base64,{font_b64}) format('woff2'); }}")).collect::<Vec<_>>().join("");
    Ok(fonts_str)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hdiag=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    color_eyre::install()?;

    let mut input_file = fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&cli.input_file)
        .wrap_err_with(|| {
            format!(
                "Failed to open file {input}",
                input = cli.input_file.display()
            )
        })?;

    let input_contents = {
        let mut buf = vec![];
        input_file
            .read_to_end(&mut buf)
            .wrap_err("Failed reading file contents")?;
        buf
    };

    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to start tokio runtime")
        .block_on(
            async move { excalidraw::get_svg_from(EXCALIDRAW_APP_ASSETS, input_contents).await },
        )
        .expect("Failed to serve folder contents");

    let raw_svg =
        String::from_utf8(result).wrap_err("Response from excalidraw was not valid UTF-8")?;

    info!(raw_svg);
    let start_str = "<style class=\"style-fonts\">";
    let start_pos = raw_svg
        .find(start_str)
        .context("SVG has no start style tag")?;
    let end_str = "</style>";
    let end_pos = raw_svg.find(end_str).context("SVG has no end style tag")?;

    let style = &raw_svg[(start_pos + start_str.len())..(end_pos + end_str.len())];
    let embedded_style =
        get_used_fonts_base64(style).wrap_err("Failed getting fonts used in file")?;
    let before_style = &raw_svg[..(start_pos + start_str.len())];
    let after_style = &raw_svg[end_pos..];
    let output = format!("{before_style}{embedded_style}{after_style}");

    let path = Path::new("out.svg");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .read(false)
        .truncate(true)
        .create(true)
        .open(path)
        .wrap_err("Failed to open output file")?;
    f.write_all(output.bytes().collect::<Vec<_>>().as_ref())
        .wrap_err("Failed to write svg to file")?;

    Ok(())
}
