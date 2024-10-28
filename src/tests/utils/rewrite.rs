#[cfg(test)]
mod tests {
    use crate::settings::rewrite::Rewrite;

    #[test]
    fn test_rewrite_path_same() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite {
            from: "/testing".to_string(),
            to: "/testing".to_string(),
        };

        let result = rewrite.rewrite_path(path.clone());

        assert_eq!(result, path);
    }

    #[test]
    fn test_rewrite_path() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite {
            from: "/testing".to_string(),
            to: "/movies".to_string(),
        };

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/movie.mkv");
    }

    #[test]
    fn test_rewrite_path_trailing() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite {
            from: "/testing/".to_string(),
            to: "/movies/".to_string(),
        };

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/movie.mkv");
    }

    #[test]
    fn test_rewrite_path_mismatch() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite {
            from: "/testing".to_string(),
            to: "/movies/".to_string(),
        };

        let result_1 = rewrite.rewrite_path(path.clone());

        let rewrite = Rewrite {
            from: "/testing/".to_string(),
            to: "/movies".to_string(),
        };

        let result_2 = rewrite.rewrite_path(path);

        assert_eq!(result_1, "/movies//movie.mkv");
        assert_eq!(result_2, "/moviesmovie.mkv");
    }

    #[test]
    fn test_rewrite_path_with_regex() {
        let path = "/testing/movie123.mkv".to_string();
        let rewrite = Rewrite {
            from: "/testing/movie(\\d+)".to_string(),
            to: "/movies/film$1".to_string(),
        };

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/film123.mkv");
    }
}
