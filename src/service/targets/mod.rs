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
///     url: "http://localhost:8096"
///     token: "<API_KEY>"
/// ```
/// or
/// ```yml
/// targets:
///   my_emby:
///     type: emby
///     url: "http://localhost:8096"
///     token: "<API_KEY>"
/// ```
///
/// See [Emby](emby::Emby) for all options
#[doc(alias("jellyfin"))]
pub mod emby;
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
///     url: "http://localhost:32400"
///     token: "<PLEX_TOKEN>"
/// ```
///
/// See [Plex](plex::Plex) for all options
pub mod plex;
