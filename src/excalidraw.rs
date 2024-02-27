use std::{net::SocketAddr, sync::Arc};

use color_eyre::{eyre::bail, Result};
use tokio::sync::mpsc;

use crate::serve_zip::{goto_page_chrome, http_serve};

pub async fn get_svg_from(
    excalidraw_assets_zip: &'static [u8],
    input_contents: Vec<u8>,
) -> Result<Vec<u8>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let addr = listener
        .local_addr()
        .expect("The listener is already bound");

    let (tx, mut rx) = mpsc::channel(1);
    let http_server = tokio::spawn(async move {
        http_serve(
            listener,
            "excalidraw",
            "input.excalidraw",
            &input_contents,
            excalidraw_assets_zip,
            Arc::new(tx),
        )
        .await
    });

    tokio::task::spawn_blocking(move || {
        goto_page_chrome(addr).expect("Failed to navigate to the page with chrome");
    });

    let Some(bytes) = rx.recv().await else {
        bail!("Sender for svg dropped unexpectedly");
    };

    http_server.abort();

    Ok(bytes)
}
