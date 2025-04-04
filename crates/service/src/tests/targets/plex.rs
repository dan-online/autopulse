#[cfg(test)]
mod tests {
    use crate::settings::targets::plex::Plex;

    #[test]
    fn test_get_search_term() {
        let plex = Plex {
            url: String::new(),
            token: String::new(),
            refresh: false,
            analyze: false,
            rewrite: None,
        };

        // Test with a path that has a file name and season directory
        let path = "/media/TV Shows/Breaking Bad/Season 1/S01E01.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Breaking Bad");

        // Test with a path that has parentheses and brackets
        let path = "/media/Movies/The Matrix (1999) [1080p]/matrix.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "The Matrix");

        // Test with a simple path
        let path = "/media/Movies/Inception/inception.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Inception");

        // Test with a directory path
        let path = "/media/TV Shows/Game of Thrones/Season 2";
        assert_eq!(plex.get_search_term(path).unwrap(), "Game of Thrones");

        // Test with multiple levels of season directories
        let path = "/media/TV Shows/Doctor Who/Season 10/Season 10 Part 2/S10E12.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Doctor Who");
    }
}
