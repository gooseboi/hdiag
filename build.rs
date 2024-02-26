#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs, io};

use zip::{ZipWriter, write::FileOptions as ZipFileOptions};
use walkdir::WalkDir;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Could not read cargo manifest dir"));
    let excalidraw_app_dir = manifest_dir.join("excalidraw-app");
    for f in ignore::Walk::new(&excalidraw_app_dir) {
        let f = f.expect("Failed to read file directory");
        let relative_path = f.path().strip_prefix(&manifest_dir).expect("Path is subpath of manifest");
        if relative_path.components().count() != 2 {
            continue;
        }
        match f.path().file_name() {
            // npm install changes package-lock for some fucking reason
            Some(s) if matches!(s.to_str(), Some("package-lock.json")) => {}
            Some(_) | None => println!("cargo:rerun-if-changed={}", f.path().display()),
        }
    }

    let excalidraw_build_dir = excalidraw_app_dir.join("dist");
    match fs::remove_dir_all(&excalidraw_build_dir) {
        Ok(()) => {},
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        e@Err(_) => e.expect("Failed to remove `excalidraw/build`"),
    }

    let mut pnpm = Command::new("pnpm")
        .arg("install")
        .stdout(Stdio::null())
        .current_dir(&excalidraw_app_dir)
        .spawn()
        .expect("Could not spawn pnpm");
    pnpm.wait().expect("Failed waiting for pnpm");

    // FIXME: Put the excalidraw stuff into the place

    let mut js_build = Command::new("pnpm")
        .args(["run", "build"])
        .stdout(Stdio::null())
        .current_dir(&excalidraw_app_dir)
        .spawn()
        .expect("Could not spawn `pnpm run build`");
    js_build.wait().expect("Failed waiting for `pnpm run build`");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Could not read out dir"));
    let bundle_path = out_dir.join("excalidraw-app.zip");
    let zip_file = fs::OpenOptions::new().create(true).truncate(true).write(true).read(false).open(bundle_path).expect("Failed to open zip file");
    let mut zw = ZipWriter::new(zip_file);
    for entry in WalkDir::new(&excalidraw_build_dir) {
        let entry = entry.expect("Failed reading entry from dir");
        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(&excalidraw_build_dir)
            .expect("Entry's path was not prefixed by build dir")
            .to_string_lossy();
        let meta = entry.metadata().expect("Failed to get entry metadata");

        if meta.is_dir() {
            zw.add_directory(rel_path, ZipFileOptions::default()).expect("Failed to add directory to zip");
        } else if meta.is_file() {
            zw.start_file(rel_path, ZipFileOptions::default()).expect("Failed to start writing file to zip");
            let mut f = fs::File::open(entry_path).expect("Failed to open file for adding to zip");
            std::io::copy(&mut f, &mut zw).expect("Failed writing file contents to zip file");
        } else {
            println!("cargo:warning=Ignoring entry {}", entry_path.display());
        }
    }
}
