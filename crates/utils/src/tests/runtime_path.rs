#[cfg(test)]
mod tests {
    use crate::{RuntimePath, RuntimePathFlavor};

    #[test]
    fn detects_runtime_flavor_and_preserves_original_rendering() {
        let unix = RuntimePath::new("/media/Shows/Episode.mkv");
        let unc = RuntimePath::new(r"\\server\media\Shows\Episode.mkv");
        let drive = RuntimePath::new(r"D:\Media\Movies\Film.mkv");

        assert_eq!(unix.flavor(), RuntimePathFlavor::Unix);
        assert_eq!(unc.flavor(), RuntimePathFlavor::Windows);
        assert_eq!(drive.flavor(), RuntimePathFlavor::Windows);
        assert_eq!(unix.to_string(), "/media/Shows/Episode.mkv");
        assert_eq!(unc.to_string(), r"\\server\media\Shows\Episode.mkv");
        assert_eq!(drive.as_str(), r"D:\Media\Movies\Film.mkv");
    }

    #[test]
    fn exposes_normal_components_filename_and_component_count() {
        let unix = RuntimePath::new("/media/Shows/Season 1/Episode.mkv");
        let drive = RuntimePath::new(r"D:\Media\Shows\Season 1\Episode.mkv");

        assert_eq!(
            unix.normal_components().collect::<Vec<_>>(),
            ["media", "Shows", "Season 1", "Episode.mkv"]
        );
        assert_eq!(
            drive.normal_components().collect::<Vec<_>>(),
            ["Media", "Shows", "Season 1", "Episode.mkv"]
        );
        assert_eq!(drive.file_name(), Some("Episode.mkv"));
        assert!(drive.component_count() > RuntimePath::new(r"D:\Media").component_count());
    }

    #[test]
    fn returns_runtime_aware_parent_for_files_and_self_for_directories() {
        let unc_file = RuntimePath::new(r"\\server\media\Shows\Episode.mkv");
        let drive_directory = RuntimePath::new(r"D:\Media\Shows");

        assert_eq!(unc_file.parent_or_self().as_str(), r"\\server\media\Shows");
        assert_eq!(drive_directory.parent_or_self().as_str(), r"D:\Media\Shows");
    }

    #[test]
    fn parent_of_a_bare_file_basename_is_empty_not_the_basename_itself() {
        let bare_file = RuntimePath::new("file.mkv");

        assert_eq!(bare_file.parent_or_self().as_str(), "");
    }

    #[test]
    fn temporary_runtime_path_preserves_input_lifetime_for_its_parent() {
        fn parent(path: &str) -> &str {
            RuntimePath::new(path).parent_or_self().as_str()
        }

        assert_eq!(
            parent(r"\\server\media\Shows\Episode.mkv"),
            r"\\server\media\Shows"
        );
    }

    #[test]
    fn classifies_the_runtime_final_component_by_extension() {
        assert!(RuntimePath::new(r"C:\Media\Film.mkv").is_file());
        assert!(!RuntimePath::new(r"C:\Media\Film.mkv").is_directory());
        assert!(RuntimePath::new(r"\\server\media\Shows").is_directory());
        assert!(RuntimePath::new("/media/archive.tar.gz").is_file());
        assert!(RuntimePath::new("/media/Shows/").is_directory());
    }

    #[test]
    fn prefix_matching_respects_components_and_nested_specificity() {
        let event = RuntimePath::new(r"\\server\media\4k\Film\Film.mkv");
        let broad = RuntimePath::new(r"\\server\media");
        let nested = RuntimePath::new(r"\\server\media\4k");
        let sibling_text = RuntimePath::new(r"\\server\media-archive");

        assert!(event.starts_with(broad));
        assert!(event.starts_with(nested));
        assert!(!sibling_text.starts_with(broad));
        assert!(nested.component_count() > broad.component_count());
    }

    #[test]
    fn default_comparison_folds_windows_ascii_case_only() {
        assert!(RuntimePath::new(r"C:\MEDIA\Shows\Episode.mkv")
            .starts_with(RuntimePath::new(r"c:\media\shows")));
        assert!(
            RuntimePath::new(r"C:\MEDIA\Film.mkv").equals(RuntimePath::new(r"c:\media\film.MKV"))
        );
        assert!(!RuntimePath::new("/MEDIA/Shows/Episode.mkv")
            .starts_with(RuntimePath::new("/media/shows")));
        assert!(!RuntimePath::new("/MEDIA/Film.mkv").equals(RuntimePath::new("/media/film.mkv")));
    }

    #[test]
    fn equals_rejects_same_length_paths_that_diverge_mid_sequence() {
        assert!(!RuntimePath::new("/media/Shows/Episode.mkv")
            .equals(RuntimePath::new("/media/Films/Episode.mkv")));
        assert!(!RuntimePath::new(r"C:\Media\Shows\Episode.mkv")
            .equals(RuntimePath::new(r"C:\Media\Films\Episode.mkv")));
    }

    #[test]
    fn explicit_case_modes_support_emby_without_crossing_flavors() {
        let windows_event = RuntimePath::new(r"C:\MEDIA\Shows\Episode.mkv");
        let windows_library = RuntimePath::new(r"c:\media\shows");
        let unix_event = RuntimePath::new("/MEDIA/Shows/Episode.mkv");
        let unix_library = RuntimePath::new("/media/shows");

        assert!(!windows_event.starts_with_case_sensitive(windows_library));
        assert!(windows_event.starts_with_ascii_case_insensitive(windows_library));
        assert!(!unix_event.starts_with_case_sensitive(unix_library));
        assert!(unix_event.starts_with_ascii_case_insensitive(unix_library));
        assert!(!windows_event.starts_with(RuntimePath::new("/MEDIA/Shows")));
        assert!(!unix_event.starts_with_ascii_case_insensitive(windows_library));
    }
}
