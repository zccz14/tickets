use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    // The frontend is embedded with rust-embed; folding the asset tree into a
    // constant forces a rebuild whenever web/dist changes, so a release binary
    // can never ship a stale UI.
    let mut files = Vec::new();
    collect_files(Path::new("web/dist"), &mut files);

    let mut digest = Sha256::new();
    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        digest.update(path.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(fs::read(path).unwrap());
        digest.update([0]);
    }
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=build.rs");

    let fingerprint = format!("{:x}", digest.finalize());
    let output = format!("const _EMBEDDED_ASSETS_FINGERPRINT: &str = {fingerprint:?};\n");
    let output_path =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("embedded_assets_fingerprint.rs");
    fs::write(output_path, output).unwrap();
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}
