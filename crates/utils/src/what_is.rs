use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum PathType {
    File,
    Directory,
}

pub fn what_is<P: AsRef<Path>>(path: P) -> PathType {
    let path_ref = path.as_ref();

    // If path ends with a path separator, consider it a directory
    if path_ref.extension().is_some() {
        PathType::File
    } else {
        PathType::Directory
    }
}
