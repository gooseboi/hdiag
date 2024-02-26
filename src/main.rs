#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::{fs, io::Read, path::PathBuf};

use clap::{Parser, ValueEnum};
use color_eyre::{eyre::WrapErr, Result};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod serve_zip;
use serve_zip::serve_zip;

const EXCALIDRAW_APP_ASSETS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-app.zip"));

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

async fn serve_excalidraw(input_contents: &[u8]) -> Result<()> {
    serve_zip(
        "excalidraw",
        "input.excalidraw",
        input_contents,
        EXCALIDRAW_APP_ASSETS,
    )
    .await
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

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to start tokio runtime")
        .block_on(async move { serve_excalidraw(&input_contents).await })
        .expect("Failed to serve folder contents");

    Ok(())
}
