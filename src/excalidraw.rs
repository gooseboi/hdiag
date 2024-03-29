use std::{fmt::Write as _, io::Cursor, net::SocketAddr, sync::Arc};

use base64::prelude::*;
use color_eyre::{
    eyre::{bail, ContextCompat, WrapErr},
    Result,
};
use tokio::sync::mpsc;
use tracing::{info, warn};
use zip::ZipArchive;

use crate::{
    cli,
    serve_zip::{goto_page_chrome, http_serve},
};

const EXCALIDRAW_APP_ASSETS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-app.zip"));

const EXCALIDRAW_FONTS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-fonts.zip"));

pub async fn get_svg_from(
    excalidraw_assets_zip: &'static [u8],
    input_contents: Vec<u8>,
    export_opts: cli::ExportOpts,
) -> Result<Vec<u8>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let addr = listener
        .local_addr()
        .expect("The listener is already bound");

    let export_opts = {
        let is_dark_mode = export_opts.theme == cli::OutputTheme::Dark;
        let scale = export_opts.scale;
        let scale = if matches!(scale, 1..=3) {
            scale
        } else {
            let clamped = scale.clamp(1, 3);
            warn!("Scale must be one of {{1, 2, 3}}, was {scale}, clamped to {clamped}");
            clamped
        };
        serde_json::json!({
            "exportBackground": export_opts.include_background,
            "exportEmbedScene": export_opts.embed_source,
            "exportWithDarkMode": is_dark_mode,
            "exportScale": scale
        })
    };
    let (tx, mut rx) = mpsc::channel(1);
    let http_server = tokio::spawn(async move {
        http_serve(
            listener,
            "excalidraw",
            "input.excalidraw",
            &input_contents,
            excalidraw_assets_zip,
            Arc::new(tx),
            export_opts,
        )
        .await
    });

    let chrome = tokio::task::spawn_blocking(move || {
        goto_page_chrome(addr).expect("Failed to navigate to the page with chrome");
    });

    let Some(bytes) = rx.recv().await else {
        bail!("Sender for svg dropped unexpectedly");
    };

    http_server.abort();
    chrome.abort();

    Ok(bytes)
}

fn split_svg_with_fonts(raw_svg: &str) -> Result<(&str, &str, &str)> {
    let start_str = "<style class=\"style-fonts\">";
    let start_pos = raw_svg
        .find(start_str)
        .context("SVG has no start style tag")?;
    let end_str = "</style>";
    let end_pos = raw_svg.find(end_str).context("SVG has no end style tag")?;

    let before = &raw_svg[..(start_pos + start_str.len())];
    let after = &raw_svg[end_pos..];
    let style = &raw_svg[(start_pos + start_str.len())..end_pos];
    Ok((before, style, after))
}

fn get_used_fonts(style: &str) -> Result<Vec<(&str, &str)>> {
    let mut slice = style;

    let pat = "excalidraw-assets/";
    let l = pat.len();
    let mut fonts = vec![];
    while let Some(idx) = slice.find(pat) {
        let start_idx = idx + l;
        let end_idx = slice[start_idx..]
            .find('"')
            .with_context(|| format!("Unexpected invalid style: {style}"))?;
        let end_idx = end_idx + start_idx;
        fonts.push(&slice[start_idx..end_idx]);
        slice = &slice[end_idx..];
    }

    let mut slice = style;

    let pat = "font-family: \"";
    let l = pat.len();
    let mut font_names = vec![];
    while let Some(idx) = slice.find(pat) {
        let start_idx = idx + l;
        let end_idx = slice[start_idx..]
            .find('"')
            .with_context(|| format!("Unexpected invalid style: {style}"))?;
        let end_idx = end_idx + start_idx;
        font_names.push(&slice[start_idx..end_idx]);
        slice = &slice[end_idx..];
    }
    Ok(font_names.into_iter().zip(fonts).collect())
}

fn embed_fonts_as_base64(style: &str) -> Result<String> {
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
                        .wrap_err("Failed to write bytes to buffer")?;
                    bytes
                }
                Err(e) => bail!("Failed to find font in zip archive: {e}"),
            };
            let mut buf = vec![0; bytes.len() * 4 / 3 + 4];

            let bytes_written = BASE64_STANDARD
                .encode_slice(&bytes, &mut buf)
                .wrap_err("Failed encoding bytes for font as base64")?;

            // shorten our vec down to just what was written
            buf.truncate(bytes_written);
            let b64 = String::from_utf8(buf).wrap_err("Output of base64 should be UTF-8")?;
            Ok((font_name, b64))
        })
        .collect();
    let fonts = fonts.wrap_err("Failed to get font file in base 64")?;
    let fonts_str =
        fonts
            .into_iter()
            .try_fold(String::new(), |mut output, (font_name, font_b64)| {
                let family = format!("font-family: \"{font_name}\";");
                let src = format!(
                    "src: url(data:font/woff2;charset=utf-8;base64,{font_b64}) format('woff2');"
                );
                write!(output, "@font-face {{ {family} {src} }}")
                    .wrap_err("Failed writing base64 encoded font to string")?;
                Ok::<String, color_eyre::Report>(output)
            })?;
    Ok(fonts_str)
}

pub async fn raw_svg(input_contents: Vec<u8>, export_opts: cli::ExportOpts) -> Result<String> {
    let result = get_svg_from(EXCALIDRAW_APP_ASSETS, input_contents, export_opts)
        .await
        .wrap_err("Failed to get svg from excalidraw app")?;

    String::from_utf8(result).wrap_err("Response from excalidraw was not valid UTF-8")
}

pub fn embed_fonts(style: &str) -> Result<String> {
    embed_fonts_as_base64(style).wrap_err("Failed getting fonts used in file")
}

pub const fn remove_fonts(_style: &str) -> String {
    String::new()
}

pub async fn render_svg(
    input_contents: Vec<u8>,
    output_format: &cli::FontFormat,
    export_opts: cli::ExportOpts,
) -> Result<String> {
    let raw_svg = raw_svg(input_contents, export_opts)
        .await
        .wrap_err("Failed getting svg from excalidraw")?;
    info!("Finished rendering raw svg");

    if *output_format == cli::FontFormat::Raw {
        return Ok(raw_svg);
    }

    let (before, fonts, after) =
        split_svg_with_fonts(&raw_svg).wrap_err("Failed splitting svg before and after")?;

    if *output_format == cli::FontFormat::Embed {
        let embedded_fonts = embed_fonts(fonts).wrap_err("Failed to embed fonts into svg")?;
        info!("Finished embedding fonts in svg");
        let output_svg = format!("{before}{embedded_fonts}{after}");
        return Ok(output_svg);
    } else if *output_format == cli::FontFormat::NoFont {
        let no_fonts = remove_fonts(fonts);
        info!("Finished removing fonts from svg");
        let output_svg = format!("{before}{no_fonts}{after}");
        return Ok(output_svg);
    }

    todo!("Unsupported output format {output_format:?} for excalidraw");
}
