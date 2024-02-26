#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use color_eyre::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const EXCALIDRAW_APP_ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/excalidraw-app.zip"));

async fn serve_app() -> Result<()> {

    todo!()
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

    Ok(())
}
