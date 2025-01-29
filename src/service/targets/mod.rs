/// Autopulse - Autopulse target
///
/// This target is used to process a file in another instance of Autopulse
///
/// # Example
///
/// ```yml
/// targets:
///   autopulse:
///     type: autopulse
///     url: http://localhost:2875
///     auth:
///       username: "admin"
///       password: "password"
/// ```
/// or
/// ```yml
/// targets:
///   autopulse:
///     type: autopulse
///     url: http://localhost:2875
///     auth:
///       username: "admin"
///       password: "password"
///     trigger: "other"
/// ```
///
/// See [Autopulse](autopulse::Autopulse) for all options
pub mod autopulse;
/// Command - Command target
///
/// This target is used to run a command to process a file
///
/// # Example
///
/// ```yml
/// targets:
///   list:
///     type: command
///     raw: "echo $FILE_PATH >> list.log"
/// ```
///
/// or
///
/// ```yml
/// targets:
///   list:
///     type: command
///     path: "/path/to/script.sh"
/// ```
///
/// See [Command](command::Command) for all options
pub mod command;
/// Emby - Emby/Jellyfin target
///
/// This target is used to refresh/scan a file in Emby/Jellyfin
///
/// # Example
///
/// ```yml
/// targets:
///   my_jellyfin:
///     type: jellyfin
///     url: http://localhost:8096
///     token: "<API_KEY>"
///     # refresh_metadata: false # To disable metadata refresh
/// ```
/// or
/// ```yml
/// targets:
///   my_emby:
///     type: emby
///     url: http://localhost:8096
///     token: "<API_KEY>"
///     # refresh_metadata: false # To disable metadata refresh
///     # metadata_refresh_mode: "validation_only" # To change metadata refresh mode
/// ```
///
/// See [Emby](emby::Emby) for all options
#[doc(alias("jellyfin"))]
pub mod emby;
/// `FileFlows` - `FileFlows` target
///
/// This target is used to process a file in `FileFlows`
///
/// # Example
///
/// ```yml
/// targets:
///   fileflows:
///     type: fileflows
///     url: http://localhost:5000
/// ```
///
/// See [`FileFlows`](fileflows::FileFlows) for all options
pub mod fileflows;
/// Plex - Plex target
///
/// This target is used to scan a file in Plex
///
/// # Example
///
/// ```yml
/// targets:
///   my_plex:
///     type: plex
///     url: http://localhost:32400
///     token: "<PLEX_TOKEN>"
/// ```
/// or
/// ```yml
/// targets:
///   my_plex:
///     type: plex
///     url: http://localhost:32400
///     token: "<PLEX_TOKEN>"
///     refresh: true
///     analyze: true
/// ```
///
/// See [Plex](plex::Plex) for all options
pub mod plex;
/// Radarr - Radarr target
///
/// This target is used to refresh/rescan a movie in Radarr
///
/// # Example
///
/// ```yml
/// targets:
///   radarr:
///     type: radarr
///     url: http://localhost:7878
///     token: "<API_KEY>"
/// ```
///
/// See [Radarr](radarr::Radarr) for all options
pub mod radarr;
/// Sonarr - Sonarr target
///
/// This target is used to refresh/rescan a series in Sonarr
///
/// # Example
///
/// ```yml
/// targets:
///   sonarr:
///     type: sonarr
///     url: http://localhost:8989
///     token: "<API_KEY>"
/// ```
///
/// See [Sonarr](sonarr::Sonarr) for all options
pub mod sonarr;
/// Tdarr - Tdarr target
///
/// This target is used to process a file in Tdarr
///
/// # Example
///
/// ```yml
/// targets:
///   tdarr:
///     type: tdarr
///     url: http://localhost:8265
///     db_id: "<LIBRARY_ID>"
/// ```
///
/// See [Tdarr](tdarr::Tdarr) for all options
pub mod tdarr;
