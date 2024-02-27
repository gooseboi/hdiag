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

            let raw_svg = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to start tokio runtime")
                .block_on(async move {
                    excalidraw::raw_svg(input_contents)
                        .await
                        .wrap_err("Failed getting svg from excalidraw")
                })?;
            info!("Finished rendering raw svg");

            let embedded_fonts_svg =
                excalidraw::embed_fonts(&raw_svg).wrap_err("Failed embedding fonts into svg")?;
            info!("Finished embedding fonts in svg");

            let mut f = fs::OpenOptions::new()
                .write(true)
                .read(false)
                .truncate(true)
                .create(true)
                .open(&cli.output_file)
                .wrap_err("Failed to open output file")?;
            f.write_all(&embedded_fonts_svg.into_bytes())
                .wrap_err("Failed to write svg to file")?;

            info!(output_path = %cli.output_file.display(), "Saved svg");
        }
        cli::FileType::Drawio => todo!("Export drawio files"),
    }

    Ok(())
}
