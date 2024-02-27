use std::{io::Cursor, net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    body::{Body, Bytes},
    extract::{self, State},
    http::{header, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use color_eyre::{eyre::eyre, Result};
use headless_chrome::{Browser, LaunchOptionsBuilder};
use tokio::{net::TcpListener, sync::mpsc::Sender, task::spawn_blocking};
use tracing::{debug, info, warn};
use zip::ZipArchive;

#[derive(Clone)]
struct AppState {
    zip_file: Arc<[u8]>,
    input_contents: Arc<[u8]>,
    input_name: Arc<str>,
    svg_channel: Arc<Sender<Vec<u8>>>,
}

type StatusResult<T> = Result<T, (StatusCode, String)>;

pub async fn http_serve(
    listener: TcpListener,
    name: &str,
    input_name: &str,
    input_contents: &[u8],
    zip_bytes: &[u8],
    svg_channel: Arc<Sender<Vec<u8>>>,
) -> Result<()> {
    let state = AppState {
        zip_file: zip_bytes.to_vec().into(),
        input_contents: input_contents.to_vec().into(),
        input_name: input_name.to_string().into(),
        svg_channel,
    };

    let app = Router::new()
        .route("/", get(fetch_root_from_zip))
        .route("/*path", get(fetch_from_zip))
        .route("/return", post(output_from_app))
        .with_state(state);

    let addr = listener
        .local_addr()
        .expect("The listener is already bound");
    debug!("Server for {name} listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn fetch_from_zip(
    State(state): State<AppState>,
    extract::Path(path): extract::Path<PathBuf>,
) -> StatusResult<impl IntoResponse> {
    fetch_path_from_zip(state, path).await
}

async fn fetch_root_from_zip(State(state): State<AppState>) -> StatusResult<impl IntoResponse> {
    fetch_path_from_zip(state, PathBuf::from("index.html")).await
}

fn find_file_in_zip(zip_file: &[u8], path: &str) -> StatusResult<Vec<u8>> {
    let mut zip = ZipArchive::new(Cursor::new(&zip_file))
        .expect("Failed to read zip archive as a zip archive");
    let res = match zip.by_name(path.as_ref()) {
        Ok(mut entry) => {
            debug!(name = entry.name(), size = entry.size(), "Found zip entry");
            let mut bytes = vec![];
            std::io::copy(&mut entry, &mut bytes).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to write bytes to buffer: {e}"),
                )
            })?;
            Ok(bytes)
        }
        Err(e) => {
            warn!("Could not find zip entry `{path}`: {e}");
            Err((StatusCode::NOT_FOUND, "File was not found".to_string()))
        }
    };
    res
}

async fn fetch_path_from_zip(state: AppState, path: PathBuf) -> StatusResult<Response<Body>> {
    let mime = path
        .extension()
        .map_or(mime::APPLICATION_OCTET_STREAM, |ext| {
            let ext = ext.to_str().expect("Extension is UTF-8");
            mime_guess::from_ext(ext).first_or_octet_stream()
        });

    let path = path
        .to_str()
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Path requested was not UTF-8".to_string(),
        ))?
        .to_string();
    debug!(%path, "Requested file from zip");
    let name = state.input_name.as_ref();
    let bytes = match path {
        n if n == name => state.input_contents.to_vec(),
        _ => spawn_blocking(move || find_file_in_zip(&state.zip_file, &path))
            .await
            .expect("Error joining thread")?,
    };

    let res = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.essence_str())
        .body(Body::from(bytes))
        .expect("Couldn't make response");
    Ok(res)
}

async fn output_from_app(State(state): State<AppState>, body: Bytes) -> StatusResult<()> {
    state
        .svg_channel
        .send(body.as_ref().to_vec())
        .await
        .unwrap_or_else(|_| panic!("Failed sending svg body through oneshot channel"));

    Ok(())
}

pub fn goto_page_chrome(addr: SocketAddr) -> Result<()> {
    let make_eyre = |e| eyre!("{e}");
    let browser =
        Browser::new(LaunchOptionsBuilder::default().headless(true).build()?).map_err(make_eyre)?;

    let tab = browser.new_tab().map_err(make_eyre)?;

    let ip = addr.ip();
    let port = addr.port();
    let url = format!("http://{ip}:{port}/index.html");
    info!(url, "Navigating to page");
    tab.navigate_to(&url).map_err(make_eyre)?;
    tab.wait_until_navigated().map_err(make_eyre)?;

    // That's it. We just need to go there, chrome loads the js, and the js
    // posts to the http server with the svg and we get the svg

    Ok(())
}
