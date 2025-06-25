#[cfg(test)]
mod tests {
    use crate::rewrite::Rewrite;

    #[test]
    fn test_rewrite_path_same() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite::single("/testing", "/testing");
        let result = rewrite.rewrite_path(path.clone());

        assert_eq!(result, path);
    }

    #[test]
    fn test_rewrite_path() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite::single("/testing", "/movies");

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/movie.mkv");
    }

    #[test]
    fn test_rewrite_path_trailing() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite::single("/testing/", "/movies/");

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/movie.mkv");
    }

    #[test]
    fn test_rewrite_path_mismatch() {
        let path = "/testing/movie.mkv".to_string();
        let rewrite = Rewrite::single("/testing", "/movies/");

        let result_1 = rewrite.rewrite_path(path.clone());

        let rewrite = Rewrite::single("/testing/", "/movies");

        let result_2 = rewrite.rewrite_path(path);

        assert_eq!(result_1, "/movies//movie.mkv");
        assert_eq!(result_2, "/moviesmovie.mkv");
    }

    #[test]
    fn test_rewrite_path_with_regex() {
        let path = "/testing/movie123.mkv".to_string();
        let rewrite = Rewrite::single("/testing/movie(\\d+)", "/movies/film$1");

        let result = rewrite.rewrite_path(path);

        assert_eq!(result, "/movies/film123.mkv");
    }

    #[test]
    fn test_rewrite_path_multiple() {
        let movies_path = "/film/film123.mkv".to_string();
        let series_path = "/tv/episode456.mkv".to_string();

        let rewrite = Rewrite::multiple(vec![("/tv/", "/series/"), ("/film/", "/movies/")]);

        let movies_result = rewrite.rewrite_path(movies_path);
        let series_result = rewrite.rewrite_path(series_path);

        assert_eq!(movies_result, "/movies/film123.mkv");
        assert_eq!(series_result, "/series/episode456.mkv");
    }

    #[test]
    fn test_rewrite_deserialize_single() {
        let json = r#"{"from": "/testing", "to": "/production"}"#;
        let rewrite: Rewrite = serde_json::from_str(json).unwrap();
        assert_eq!(rewrite.rewrites.len(), 1);
        assert_eq!(rewrite.rewrites[0].from, "/testing");
        assert_eq!(rewrite.rewrites[0].to, "/production");
    }

    #[test]
    fn test_rewrite_deserialize_multiple() {
        let json = r#"
        [
            {"from": "/testing", "to": "/production"},
            {"from": "^/old/path/(.*)$", "to": "/new/path/$1"}
        ]"#;

        let rewrite: Rewrite = serde_json::from_str(json).unwrap();

        assert_eq!(rewrite.rewrites.len(), 2);
        assert_eq!(rewrite.rewrites[0].from, "/testing");
        assert_eq!(rewrite.rewrites[0].to, "/production");
        assert_eq!(rewrite.rewrites[1].from, "^/old/path/(.*)$");
        assert_eq!(rewrite.rewrites[1].to, "/new/path/$1");
    }
}
