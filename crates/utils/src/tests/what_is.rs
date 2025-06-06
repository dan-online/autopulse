#[cfg(test)]
mod tests {
    use crate::what_is::*;

    #[test]
    fn test_files_with_extensions() {
        assert_eq!(what_is("file.txt"), PathType::File);
        assert_eq!(what_is("/absolute/path/file.rs"), PathType::File);
        assert_eq!(what_is("./relative/path/file.md"), PathType::File);
        assert_eq!(what_is("../parent/file.json"), PathType::File);
        assert_eq!(what_is("multiple.extension.points.txt"), PathType::File);
    }

    #[test]
    fn test_directories_without_extensions() {
        assert_eq!(what_is("directory"), PathType::Directory);
        assert_eq!(what_is("/absolute/path/directory"), PathType::Directory);
        assert_eq!(what_is("./relative/path/directory"), PathType::Directory);
        assert_eq!(what_is("../parent/directory"), PathType::Directory);
        assert_eq!(what_is("../absolute/directory/"), PathType::Directory);
    }

    #[test]
    fn test_edge_cases() {
        // Empty path and special cases
        assert_eq!(what_is(""), PathType::Directory);
        assert_eq!(what_is("."), PathType::Directory);
        assert_eq!(what_is(".."), PathType::Directory);
    }

    #[test]
    fn test_path_with_trailing_separator() {
        assert_eq!(what_is("directory/"), PathType::Directory);
    }

    #[test]
    fn test_squash_directory() {
        assert_eq!(squash_directory("file.txt").as_os_str(), "");
        assert_eq!(squash_directory("directory/").as_os_str(), "directory/");
        assert_eq!(squash_directory("directory").as_os_str(), "directory");
        assert_eq!(
            squash_directory("/absolute/path/file.rs").as_os_str(),
            "/absolute/path"
        );
        assert_eq!(
            squash_directory("./relative/path/file.md").as_os_str(),
            "./relative/path"
        );
    }
}
