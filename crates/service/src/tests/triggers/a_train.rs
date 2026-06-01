#![cfg(test)]
mod tests {
    use crate::settings::triggers::a_train::ATrainRequest;
    use crate::settings::triggers::TriggerRequest;

    #[test]
    fn test_from_json_created() {
        let json = serde_json::json!({
            "created": [
                "/Movies/Interstellar (2014)",
                "/TV/Legion/Season 1"
            ]
        });
        let req = ATrainRequest::from_json(json).unwrap();
        assert_eq!(
            req.paths(),
            vec![
                ("/Movies/Interstellar (2014)".to_string(), true),
                ("/TV/Legion/Season 1".to_string(), true)
            ]
        );
    }

    #[test]
    fn test_from_json_deleted() {
        let json = serde_json::json!({
            "deleted": [
                "/Movies/Wonder Woman 1984 (2020)",
                "/Movies/Mortal Kombat (2021)"
            ]
        });
        let req = ATrainRequest::from_json(json).unwrap();
        assert_eq!(
            req.paths(),
            vec![
                ("/Movies/Wonder Woman 1984 (2020)".to_string(), false),
                ("/Movies/Mortal Kombat (2021)".to_string(), false)
            ]
        );
    }

    #[test]
    fn test_from_json_both() {
        let json = serde_json::json!({
            "created": ["/TV/Legion/Season 1"],
            "deleted": ["/TV/Legion/Season 1"]
        });
        let req = ATrainRequest::from_json(json).unwrap();
        assert_eq!(
            req.paths(),
            vec![
                ("/TV/Legion/Season 1".to_string(), true),
                ("/TV/Legion/Season 1".to_string(), false)
            ]
        );
    }

    #[test]
    fn test_from_json_empty() {
        let json = serde_json::json!({});
        let req = ATrainRequest::from_json(json).unwrap();
        assert_eq!(req.paths(), vec![]);
    }

    #[test]
    fn test_paths_dedup_within_and_across_buckets() {
        // A buggy or replayed payload could ship duplicates. We dedup on
        // `(path, search)` while preserving order — including across the
        // created/deleted boundary, so a path that appears in both still
        // produces one created and one deleted entry.
        let json = serde_json::json!({
            "created": [
                "/TV/Legion/Season 1",
                "/TV/Legion/Season 1",
                "/Movies/Dune (2021)",
            ],
            "deleted": [
                "/TV/Legion/Season 1",
                "/Movies/Dune (2021)",
                "/Movies/Dune (2021)",
            ]
        });
        let req = ATrainRequest::from_json(json).unwrap();
        assert_eq!(
            req.paths(),
            vec![
                ("/TV/Legion/Season 1".to_string(), true),
                ("/Movies/Dune (2021)".to_string(), true),
                ("/TV/Legion/Season 1".to_string(), false),
                ("/Movies/Dune (2021)".to_string(), false),
            ]
        );
    }
}
