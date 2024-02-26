#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs, io};

use zip::ZipWriter;
use walkdir::WalkDir;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Could not read cargo manifest dir"));
    let node_dir = manifest_dir.join("node");
    for f in ignore::Walk::new(&node_dir) {
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

    let node_build_dir = node_dir.join("build");
    match fs::remove_dir_all(&node_build_dir) {
        Ok(()) => {},
        Err(e) if e.kind() == io::ErrorKind::NotFound => {},
        e@Err(_) => e.expect("Failed to remove `node/build`"),
    }

    let mut npm = Command::new("npm")
        .arg("install")
        .stdout(Stdio::null())
        .current_dir(&node_dir)
        .spawn()
        .expect("Could not spawn npm");
    npm.wait().expect("Failed waiting for npm");

    // FIXME: Put the excalidraw stuff into the place

    let mut node = Command::new("npm")
        .args(["run", "build"])
        .stdout(Stdio::null())
        .current_dir(&node_dir)
        .spawn()
        .expect("Could not spawn node");
    node.wait().expect("Failed waiting for node");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Could not read out dir"));
    let bundle_path = out_dir.join("excalidraw-app.zip");
    let zip_file = fs::OpenOptions::new().create(true).truncate(true).write(true).read(false).open(bundle_path).expect("Failed to open zip file");
    let mut zw = ZipWriter::new(zip_file);
    for entry in WalkDir::new(&node_build_dir) {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(&node_build_dir)
            .unwrap()
            .to_string_lossy();
        let meta = entry.metadata().unwrap();

        if meta.is_dir() {
            zw.add_directory(rel_path, Default::default()).unwrap();
        } else if meta.is_file() {
            zw.start_file(rel_path, Default::default()).unwrap();
            std::io::copy(&mut fs::File::open(entry_path).unwrap(), &mut zw).unwrap();
        } else {
            println!("cargo:warning=Ignoring entry {}", entry_path.display());
        }
    }
}
