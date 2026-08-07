#[cfg(test)]
mod tests {
    use crate::{
        settings::triggers::sportarr::SportarrRequest, settings::triggers::TriggerRequest,
    };

    #[test]
    fn test_from_json_test() {
        let json = serde_json::json!({
            "eventType": "Test"
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();

        assert!(matches!(sportarr_request, SportarrRequest::Test));
    }

    #[test]
    fn test_from_json_download() {
        let json = serde_json::json!({
            "eventType": "Download",
            "episodeFile": { "relativePath": "NFL.2026.08.04.mkv" },
            "series": {
                "title": "NFL",
                "path": "/Sports/NFL/Season 2026"
            }
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();

        if let SportarrRequest::Download {
            episode_file,
            series,
            ..
        } = sportarr_request.clone()
        {
            assert_eq!(episode_file.unwrap().relative_path, "NFL.2026.08.04.mkv");
            assert_eq!(series.path.as_deref(), Some("/Sports/NFL/Season 2026"));
            assert_eq!(
                sportarr_request.paths(),
                vec![(
                    "/Sports/NFL/Season 2026/NFL.2026.08.04.mkv".to_string(),
                    true
                )]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    // Real payload captured from a live Sportarr instance (dev branch,
    // 2026-08-10; note the series.tsdbId cross-reference) after a POST /api/leagues/{id}/rename call
    // that renamed one file. Unlike Sonarr's Rename event (a per-file
    // previousPath/relativePath list), Sportarr sends the covering
    // directory of the whole renamed batch as series.path plus a
    // renamedCount - there's no single file to point at for a rename
    // batch. This is the exact shape dan-online reported 500ing when
    // Sportarr triggers were routed through the Sonarr parser.
    #[test]
    fn test_from_json_real_rename_payload() {
        let json = serde_json::json!({
            "eventType": "Rename",
            "title": "Renamed 1 file(s)",
            "message": "Scope: NFL",
            "applicationUrl": "",
            "instanceName": "Sportarr",
            "series": {
                "id": 2,
                "externalId": "ev-990002",
                "tsdbId": "2368401",
                "title": "NFL 2026-08-09 Team A vs Team B",
                "path": "/media/sports/NFL",
                "league": "NFL",
                "sport": "American Football"
            },
            "renamedCount": 1
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();

        if let SportarrRequest::Rename { series } = sportarr_request.clone() {
            assert_eq!(series.path.as_deref(), Some("/media/sports/NFL"));
            assert_eq!(
                sportarr_request.paths(),
                vec![("/media/sports/NFL".to_string(), true)]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    // Real payload captured from a live Sportarr instance (dev branch,
    // 2026-08-10; note the series.tsdbId cross-reference) after a DELETE /api/events/{id} call.
    // SeriesDelete in Sportarr only removes the database record - it never
    // deletes files (that's a separate EpisodeFileDelete event) - so the
    // `series` object carries no `path` field at all, only id/title.
    // Sonarr's Series struct treats `path` as required, which is exactly
    // what made this 500 when routed through the Sonarr parser.
    #[test]
    fn test_from_json_real_series_delete_payload() {
        let json = serde_json::json!({
            "eventType": "SeriesDelete",
            "title": "Event deleted: NFL 2026-08-05 Team C vs Team D",
            "message": "The event was removed from the library.",
            "applicationUrl": "",
            "instanceName": "Sportarr",
            "series": {
                "id": 2,
                "externalId": "ev-990006",
                "tsdbId": "2368406",
                "title": "NFL 2026-08-05 Team C vs Team D",
                "league": "NFL",
                "sport": "American Football"
            }
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();

        if let SportarrRequest::SeriesDelete { series } = sportarr_request.clone() {
            assert_eq!(series.path, None);
            assert_eq!(sportarr_request.paths(), vec![]);
        } else {
            panic!("Unexpected variant");
        }
    }

    // Real payload captured from a live Sportarr instance (dev branch,
    // 2026-08-10; note the series.tsdbId cross-reference) after a DELETE /api/events/{id}/files call
    // that removed two files. Unlike Sonarr (a single `episodeFile`),
    // Sportarr always reports file deletions - manual, retention-driven,
    // or an upgrade's replaced file - through the plural `deletedFiles`
    // list, same as `Download` uses for an upgrade's replaced file. There
    // is no `episodeFile` field on this event at all, which would have
    // 500'd the same way Rename and SeriesDelete did.
    #[test]
    fn test_from_json_real_episode_file_delete_payload() {
        let json = serde_json::json!({
            "eventType": "EpisodeFileDelete",
            "title": "Deleted: NFL 2026-08-06 Team E vs Team F",
            "message": "File: file1.mkv",
            "applicationUrl": "",
            "instanceName": "Sportarr",
            "series": {
                "id": 1,
                "externalId": "ev-990003",
                "tsdbId": "2368403",
                "title": "NFL 2026-08-06 Team E vs Team F",
                "path": "/media/sports/NFL",
                "league": "NFL",
                "sport": "American Football"
            },
            "deletedFiles": [
                { "relativePath": "file1.mkv", "path": "/media/sports/NFL/file1.mkv", "size": 111 },
                { "relativePath": "file2.mkv", "path": "/media/sports/NFL/file2.mkv", "size": 222 }
            ]
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();

        if let SportarrRequest::EpisodeFileDelete {
            deleted_files,
            series,
        } = sportarr_request.clone()
        {
            assert_eq!(deleted_files.len(), 2);
            assert_eq!(series.path.as_deref(), Some("/media/sports/NFL"));
            assert_eq!(
                sportarr_request.paths(),
                vec![
                    ("/media/sports/NFL/file1.mkv".to_string(), false),
                    ("/media/sports/NFL/file2.mkv".to_string(), false)
                ]
            );
        } else {
            panic!("Unexpected variant");
        }
    }

    #[test]
    fn test_from_json_unknown_event_type() {
        let json = serde_json::json!({
            "eventType": "Grab",
            "series": { "title": "NFL", "path": "/Sports/NFL" }
        });

        let sportarr_request = SportarrRequest::from_json(json).unwrap();
        assert!(matches!(sportarr_request, SportarrRequest::Other));
        assert_eq!(sportarr_request.paths(), vec![]);
    }
}
