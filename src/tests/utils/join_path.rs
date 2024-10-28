#[cfg(test)]
mod tests {
    use crate::utils::join_path::join_path;

    #[test]
    fn test_join_path() {
        let root = "/root";
        let relative = "/relative";

        assert_eq!(join_path(root, relative), "/root/relative");
    }

    #[test]
    fn test_join_path_no_slash() {
        let root = "/root";
        let relative = "relative";

        assert_eq!(join_path(root, relative), "/root/relative");
    }

    #[test]
    fn test_join_path_no_root() {
        let root = "";
        let relative = "/relative";

        assert_eq!(join_path(root, relative), "/relative");
    }
}
