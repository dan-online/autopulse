// build.rs
use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(["describe", "--always", "--tags"])
        .output()
        .expect("Failed to execute git command");

    let git_hash = String::from_utf8(output.stdout).unwrap();

    println!("cargo:rustc-env=GIT_REVISION={}", git_hash.trim());
}
