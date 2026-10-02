#[cfg(test)]
mod tests {
    use crate::settings::triggers::{tdarr::TdarrRequest, Trigger, TriggerRequest};
    use serde_json::json;

    #[test]
    fn test_path_only() {
        let request = TdarrRequest::from_json(json!({ "path": "/media/movie.mp4" })).unwrap();
        assert_eq!(
            request.paths(),
            vec![("/media/movie.mp4".to_string(), true)]
        );
    }

    #[test]
    fn test_container_change() {
        let request = TdarrRequest::from_json(json!({
            "path": "/media/movie.mp4",
            "original_path": "/media/movie.mkv"
        }))
        .unwrap();
        assert_eq!(
            request.paths(),
            vec![
                ("/media/movie.mp4".to_string(), true),
                ("/media/movie.mkv".to_string(), false)
            ]
        );
    }

    #[test]
    fn test_equal_paths_deduplicated() {
        let request = TdarrRequest::from_json(json!({
            "path": "/media/movie.mkv",
            "original_path": "/media/movie.mkv"
        }))
        .unwrap();
        assert_eq!(
            request.paths(),
            vec![("/media/movie.mkv".to_string(), true)]
        );
    }

    #[test]
    fn test_dir_preserves_trailing_slash() {
        let request = TdarrRequest::from_json(json!({ "dir": "/media/Show/Season 1/" })).unwrap();
        assert_eq!(
            request.paths(),
            vec![("/media/Show/Season 1/".to_string(), true)]
        );
    }

    #[test]
    fn test_missing_paths_rejected() {
        for body in [
            json!({}),
            json!({ "path": "" }),
            json!({ "path": " \t", "original_path": "\n", "dir": " " }),
            json!({ "path": null, "original_path": null, "dir": null }),
        ] {
            assert!(TdarrRequest::from_json(body).is_err());
        }
    }

    #[test]
    fn test_empty_template_values_ignored_and_nonempty_values_preserved() {
        let request = TdarrRequest::from_json(json!({
            "path": " \t",
            "original_path": " /media/movie.mkv ",
            "dir": ""
        }))
        .unwrap();
        assert_eq!(
            request.paths(),
            vec![(" /media/movie.mkv ".to_string(), false)]
        );
    }

    #[test]
    fn test_extra_fields_ignored_and_path_order() {
        let request = TdarrRequest::from_json(json!({
            "path": "/media/movie.mp4",
            "original_path": "/media/movie.mkv",
            "dir": "/media/",
            "libraryId": "ignored",
            "metadata": { "codec": "hevc" }
        }))
        .unwrap();
        assert_eq!(
            request.paths(),
            vec![
                ("/media/movie.mp4".to_string(), true),
                ("/media/movie.mkv".to_string(), false),
                ("/media/".to_string(), true)
            ]
        );
    }

    #[test]
    fn test_trigger_deserialization_and_unknown_event() {
        let trigger: Trigger = serde_json::from_value(json!({ "type": "tdarr" })).unwrap();
        assert!(matches!(trigger, Trigger::Tdarr(_)));
        assert_eq!(
            trigger
                .paths(json!({ "path": "/media/movie.mp4" }))
                .unwrap(),
            (
                "unknown".to_string(),
                vec![("/media/movie.mp4".to_string(), true)]
            )
        );
    }
}
