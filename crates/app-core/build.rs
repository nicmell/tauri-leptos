fn main() {
    // The embedded frontend bundle. Ensure the folder exists so rust-embed's
    // derive works on a fresh clone, and re-embed when Trunk rewrites it
    // (content-hashed filenames change the file set on every build).
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    let _ = std::fs::create_dir_all(&dist);
    println!("cargo:rerun-if-changed={}", dist.display());
}
