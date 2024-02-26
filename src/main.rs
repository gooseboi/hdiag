#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use color_eyre::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod serve_zip;
use serve_zip::serve_zip;

const EXCALIDRAW_APP_ASSETS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-app.zip"));

async fn serve_excalidraw() -> Result<()> {
    serve_zip("excalidraw", "input.excalidraw", todo!(), EXCALIDRAW_APP_ASSETS).await
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hdiag=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    color_eyre::install()?;

    println!("{}", EXCALIDRAW_APP_ASSETS.len());
    println!("Hello, world!");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move { serve_excalidraw().await })
        .unwrap();

    Ok(())
}
