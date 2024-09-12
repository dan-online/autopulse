/// Lidarr - Lidarr trigger
///
/// This trigger is used to process a file from Lidarr
///
/// # Example
///
/// ```yml
/// triggers:
///   my_lidarr:
///     type: lidarr
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_lidarr:
///     type: lidarr
///     rewrite:
///       from: "/downloads/music"
///       to: "/music"
///     timer:
///       wait: 30
/// ```
///
/// See [Lidarr](crate::utils::settings::Trigger::Lidarr) for all options
pub mod lidarr;
/// Manual - Manual trigger
///
/// This trigger is used to manually process a file. Often used when implementing a custom trigger
///
/// Note: A default route of `/triggers/manual` is provided
///
/// # Example
///
/// ```yml
/// triggers:
///   my_manual:
///     type: manual
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_manual:
///     type: manual
///     rewrite:
///       from: "/downloads/stuff"
///       to: "/stuff"
///     timer:
///       wait: 30
/// ```
///
/// See [Manual](crate::utils::settings::Trigger::Manual) for all options
/// and
/// [ManualQueryParams](manual::ManualQueryParams) for query parameters
pub mod manual;
/// Notify - Notify trigger
///
/// Cross-platform monitoring for a directory to process based on file events
///
/// # Example
///
/// ```yml
/// triggers:
///   my_notify:
///     type: notify
///     paths:
///       - "/path/to/monitor"
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_notify:
///     type: notify
///     paths:
///       - "/downloads"
///     recursive: false
///     rewrite:
///       from: "/downloads"
///       to: "/media"
///     timer:
///       wait: 30
/// ```
///
/// See [Notify](notify::Notify) for all options
pub mod notify;
/// Radarr - Radarr trigger
///
/// This trigger is used to process a file from Radarr
///
/// # Example
///
/// ```yml
/// triggers:
///   my_radarr:
///     type: radarr
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_radarr:
///     type: radarr
///     rewrite:
///       from: "/downloads/movies"
///       to: "/movies"
///     timer:
///       wait: 30
/// ```
///
/// See [Radarr](crate::utils::settings::Trigger::Radarr) for all options
pub mod radarr;
/// Readarr - Readarr trigger
///
/// This trigger is used to process a file from Readarr
///
/// # Example
///
/// ```yml
/// triggers:
///   my_readarr:
///     type: readarr
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_readarr:
///     type: readarr
///     rewrite:
///       from: "/downloads/books"
///       to: "/books"
///     timer:
///       wait: 30
/// ```
///
/// See [Readarr](crate::utils::settings::Trigger::Readarr) for all options
pub mod readarr;
/// Sonarr - Sonarr trigger
///
/// This trigger is used to process a file from Sonarr
///
/// # Example
///
/// ```yml
/// triggers:
///   my_sonarr:
///     type: sonarr
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_sonarr:
///     type: sonarr
///     rewrite:
///       from: "/downloads/shows"
///       to: "/shows"
///     timer:
///       wait: 30
/// ```
///
/// See [Sonarr](crate::utils::settings::Trigger::Sonarr) for all options
pub mod sonarr;
