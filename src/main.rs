#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::{
    fs,
    io::{Read as _, Write as _},
};

use color_eyre::{eyre::WrapErr as _, Result};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

mod cli;
mod excalidraw;
mod serve_zip;

fn main() -> Result<()> {
    let cli = cli::Opts::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hdiag=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    color_eyre::install()?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .wrap_err("Failed to build tokio runtime")?;
    match cli.input_type {
        cli::FileType::Excalidraw => {
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

            let svg = rt.block_on(async move {
                excalidraw::render_svg(input_contents, &cli.output_format, cli.export)
                    .await
                    .wrap_err("Failed rendering excalidraw svg")
            })?;

            let mut f = fs::OpenOptions::new()
                .write(true)
                .read(false)
                .truncate(true)
                .create(true)
                .open(&cli.output_file)
                .wrap_err("Failed to open output file")?;
            f.write_all(&svg.into_bytes())
                .wrap_err("Failed to write svg to file")?;

            info!(output_path = %cli.output_file.display(), "Saved svg");
        }
        cli::FileType::Drawio => todo!("Export drawio files"),
    }

    Ok(())
}
