use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
    process::Command,
};

fn main() {
    let git_hash = Command::new("git")
        .args(["describe", "--always", "--tags"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_REVISION={}", git_hash.trim());

    // Cache-busting suffix for /ui/static/*: a content hash of the embedded
    // assets, so it changes only when an asset changes (paired with the
    // rerun-if-changed directives below). Stable across no-op rebuilds, which
    // keeps browser caches warm when the UI hasn't actually changed.
    println!("cargo:rerun-if-changed=build.rs");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_dir(Path::new("static"), &mut hasher);
    println!("cargo:rustc-env=ASSETS_VERSION={:x}", hasher.finish());
}

/// Hashes every file under `dir` (path + bytes) and emits a
/// `rerun-if-changed` for each, so the stamp tracks asset content. Walks
/// recursively to cover nested dirs like `static/fonts/`. Panics if the
/// directory is missing (`static/` is required for the UI to build).
fn hash_dir(dir: &Path, hasher: &mut impl Hasher) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("missing required asset dir {}: {e}", dir.display()))
        .map(|e| e.expect("read static/ entry").path())
        .collect();
    // Sort so the hash is independent of filesystem iteration order.
    entries.sort();
    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            hash_dir(&path, hasher);
        } else {
            path.to_string_lossy().hash(hasher);
            fs::read(&path)
                .unwrap_or_else(|e| panic!("read asset {}: {e}", path.display()))
                .hash(hasher);
        }
    }
}
