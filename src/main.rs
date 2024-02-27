#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::{
    fs,
    io::{Read, Write as _},
    path::{Path, PathBuf},
};

use clap::{Parser, ValueEnum};
use color_eyre::{
    eyre::WrapErr,
    Result,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod excalidraw;
mod serve_zip;

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

    let raw_svg = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to start tokio runtime")
        .block_on(async move {
            excalidraw::raw_svg(input_contents)
                .await
                .wrap_err("Failed getting svg from excalidraw")
        })?;
    info!(raw_svg);
    let embedded_fonts_svg =
        excalidraw::embed_fonts(&raw_svg).wrap_err("Failed embedding fonts into svg")?;

    let path = Path::new("out.svg");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .read(false)
        .truncate(true)
        .create(true)
        .open(path)
        .wrap_err("Failed to open output file")?;
    f.write_all(&raw_svg.into_bytes())
        .wrap_err("Failed to write svg to file")?;

    let path = Path::new("out-embedded.svg");
    let mut f = fs::OpenOptions::new()
        .write(true)
        .read(false)
        .truncate(true)
        .create(true)
        .open(path)
        .wrap_err("Failed to open output file")?;
    f.write_all(&embedded_fonts_svg.into_bytes())
        .wrap_err("Failed to write svg to file")?;

    Ok(())
}
