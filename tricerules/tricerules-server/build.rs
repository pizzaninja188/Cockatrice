use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn sources(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read engine sources") {
            sources(&entry.expect("source entry").path(), files);
        }
    } else {
        files.push(path.to_owned());
    }
}

fn main() {
    let workspace = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let mut files = vec![];
    for package in [
        "tricerules-core",
        "tricerules-cards",
        "tricerules-proto",
        "tricerules-server",
    ] {
        let source = workspace.join(package).join("src");
        println!("cargo:rerun-if-changed={}", source.display());
        sources(&source, &mut files);
        files.push(workspace.join(package).join("Cargo.toml"));
    }
    files.extend([
        workspace.join("Cargo.lock"),
        workspace.join("Cargo.toml"),
        workspace.join("tricerules-proto/build.rs"),
        workspace.join("tricerules-server/build.rs"),
        workspace.join("../libcockatrice_protocol/libcockatrice/protocol/pb/ruled_v1.proto"),
        workspace
            .join("../libcockatrice_protocol/libcockatrice/protocol/pb/ruled_diagnostics.proto"),
    ]);
    files.sort();
    let mut hash = Sha256::new();
    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
        hash.update(
            file.strip_prefix(&workspace)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
                .as_bytes(),
        );
        hash.update([0]);
        // Checkout line endings must not change the identity of otherwise identical source.
        hash.update(
            fs::read_to_string(&file)
                .expect("UTF-8 source")
                .replace("\r\n", "\n")
                .as_bytes(),
        );
        hash.update([0]);
    }
    println!(
        "cargo:rustc-env=TRICERULES_BUILD_FINGERPRINT=sha256:{:x}",
        hash.finalize()
    );
}
