use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum PathType {
    File,
    Directory,
}

pub fn what_is<P: AsRef<Path>>(path: P) -> PathType {
    let path_ref = path.as_ref();

    if path_ref.extension().is_some() {
        PathType::File
    } else {
        PathType::Directory
    }
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    matches!(what_is(path), PathType::File)
}

pub fn is_directory<P: AsRef<Path>>(path: P) -> bool {
    matches!(what_is(path), PathType::Directory)
}

pub fn squash_directory<P: AsRef<Path>>(path: P) -> PathBuf {
    let path_ref = path.as_ref();

    match what_is(path_ref) {
        PathType::File => path_ref.parent().unwrap_or(path_ref).to_path_buf(),
        PathType::Directory => path_ref.to_path_buf(),
    }
}
