#[doc(hidden)]
pub mod check_auth;
#[doc(hidden)]
pub mod checksum;
#[doc(hidden)]
pub mod generate_uuid;
#[doc(hidden)]
pub mod get_timestamp;
#[doc(hidden)]
pub mod join_path;
#[doc(hidden)]
pub mod logs;
#[doc(hidden)]
pub mod sify;

/// Arguments for CLI
///
/// ```
/// $ autopulse --help
/// ```
///
/// See [Args](cli::Args) for all options
pub mod cli;
