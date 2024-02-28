#![deny(
    clippy::enum_glob_use,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs, io};

use walkdir::WalkDir;
use zip::{write::FileOptions as ZipFileOptions, ZipWriter};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Could not read cargo manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Could not read out dir"));

    generate_excalidraw_assets(&manifest_dir, &out_dir);
}

fn gen_zip_file<P, Q>(source_path: P, zip_path: Q)
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let zip_file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .read(false)
        .open(zip_path)
        .expect("Failed to open zip file");
    let mut zw = ZipWriter::new(zip_file);
    for entry in WalkDir::new(&source_path) {
        let entry = entry.expect("Failed reading entry from dir");
        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(&source_path)
            .expect("Entry's path was not prefixed by build dir")
            .to_string_lossy()
            // Fucking windows
            .replace('\\', "/");
        let meta = entry.metadata().expect("Failed to get entry metadata");

        if meta.is_dir() {
            zw.add_directory(rel_path, ZipFileOptions::default())
                .expect("Failed to add directory to zip");
        } else if meta.is_file() {
            zw.start_file(rel_path, ZipFileOptions::default())
                .expect("Failed to start writing file to zip");
            let mut f = fs::File::open(entry_path).expect("Failed to open file for adding to zip");
            std::io::copy(&mut f, &mut zw).expect("Failed writing file contents to zip file");
        } else {
            println!("cargo:warning=Ignoring entry {}", entry_path.display());
        }
    }
}

fn rmdir_force<P>(path: P)
where
    P: AsRef<Path>,
{
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        e @ Err(_) => e.expect("Failed to remove `excalidraw/build`"),
    }
}

fn generate_excalidraw_assets(manifest_dir: &Path, out_dir: &Path) {
    let excalidraw_app_dir = manifest_dir.join("excalidraw-app");
    for f in ignore::Walk::new(&excalidraw_app_dir) {
        let f = f.expect("Failed to read file directory");
        let relative_path = f
            .path()
            .strip_prefix(manifest_dir)
            .expect("Path is subpath of manifest");
        if relative_path.components().count() != 2 {
            continue;
        }
        match f.path().file_name() {
            // npm install changes package-lock for some fucking reason
            // the public one is because we copy the excalidraw assets there
            Some(s) if matches!(s.to_str(), Some("package-lock.json" | "public")) => {}
            Some(_) | None => println!("cargo:rerun-if-changed={}", f.path().display()),
        }
    }

    let excalidraw_build_dir = excalidraw_app_dir.join("dist");

    let mut pnpm = Command::new("pnpm")
        .arg("install")
        .stdout(Stdio::null())
        .current_dir(&excalidraw_app_dir)
        .spawn()
        .expect("Could not spawn pnpm");
    pnpm.wait().expect("Failed waiting for pnpm");

    let excalidraw_assets_dir =
        excalidraw_app_dir.join("node_modules/@excalidraw/excalidraw/dist/excalidraw-assets");
    let assets_dir = excalidraw_app_dir.join("public/excalidraw-assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets output dir");
    let font_output_dir = out_dir.join("fonts_temp");
    fs::create_dir_all(&font_output_dir).expect("Failed to create font output dir");
    for entry in WalkDir::new(&excalidraw_assets_dir) {
        let entry = entry.expect("Failed to read excalidraw_assets dir entry");
        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(&excalidraw_assets_dir)
            .expect("Entry path was not prefixed by excalidraw_assets");

        let ext = rel_path
            .extension()
            .map(|s| s.to_str().expect("Extension was not UTF-8"));
        if matches!(ext, Some("woff2")) {
            let name = rel_path
                .file_name()
                .expect("File had extension but no file_name");
            let out_path = font_output_dir.join(name);
            fs::copy(entry_path, out_path).expect("Failed to copy font to output dir");
        }

        let meta = entry.metadata().expect("Failed to get entry metadata");
        if meta.is_dir() {
            // works like mkdir -p
            fs::create_dir_all(assets_dir.join(rel_path))
                .expect("Failed creating dir in public dir");
        } else if meta.is_file() {
            let p = assets_dir.join(rel_path);
            fs::copy(entry_path, p).expect("Failed to copy file to public dir");
        } else {
            println!("cargo:warning=Ignoring entry {}", entry_path.display());
        }
    }
    let font_zip_file = out_dir.join("excalidraw-fonts.zip");
    gen_zip_file(&font_output_dir, font_zip_file);
    rmdir_force(&font_output_dir);

    let mut js_build = Command::new("pnpm")
        .args(["run", "build"])
        .stdout(Stdio::null())
        .current_dir(&excalidraw_app_dir)
        .spawn()
        .expect("Could not spawn `pnpm run build`");
    js_build
        .wait()
        .expect("Failed waiting for `pnpm run build`");

    let bundle_path = out_dir.join("excalidraw-app.zip");
    gen_zip_file(excalidraw_build_dir, bundle_path);
}
