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

    // Content hash → cache-buster on /ui/static/*. Stable across no-op rebuilds.
    println!("cargo:rerun-if-changed=build.rs");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_dir(Path::new("static"), &mut hasher);
    println!("cargo:rustc-env=ASSETS_VERSION={:x}", hasher.finish());
}

fn hash_dir(dir: &Path, hasher: &mut impl Hasher) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("missing required asset dir {}: {e}", dir.display()))
        .map(|e| e.expect("read static/ entry").path())
        .collect();
    // Sort for hash determinism across filesystems.
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
