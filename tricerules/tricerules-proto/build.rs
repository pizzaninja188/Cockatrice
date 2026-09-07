//! Generates Rust from libcockatrice_protocol ruled*.proto (proto3).

use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let proto_root = manifest_dir.join("../../libcockatrice_protocol/libcockatrice/protocol/pb");
    let ruled_v1 = proto_root.join("ruled_v1.proto");
    let diagnostics = proto_root.join("ruled_diagnostics.proto");

    // Proto lives outside this crate; ensure edits invalidate cached prost output (e.g. release/ vs debug/ drift).
    println!("cargo:rerun-if-changed={}", ruled_v1.display());
    println!("cargo:rerun-if-changed={}", diagnostics.display());

    let mut config = prost_build::Config::new();
    config.btree_map(["."]);
    config.file_descriptor_set_path(
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ruled-descriptors.bin"),
    );
    config.type_attribute(".", "#[derive(serde::Serialize)]");
    config.compile_protos(&[ruled_v1, diagnostics], &[proto_root])?;
    Ok(())
}
