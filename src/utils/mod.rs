#[doc(hidden)]
pub mod check_auth;
#[doc(hidden)]
pub mod checksum;
#[doc(hidden)]
pub mod default_true;
#[doc(hidden)]
pub mod generate_uuid;
#[doc(hidden)]
pub mod get_timestamp;
#[doc(hidden)]
pub mod join_path;
#[doc(hidden)]
pub mod logs;
#[doc(hidden)]
pub mod rewrite;
#[doc(hidden)]
pub mod sify;

/// Configuration settings
///
/// Used to configure the service.
///
/// Can be defined in 2 ways:
/// - Config file
///   - `config.{json,toml,yaml,json5,ron,ini}` in the current directory
/// - Environment variables
///   - `AUTOPULSE__{SECTION}__{KEY}` (e.g. `AUTOPULSE__APP__DATABASE_URL`)
///
/// See [Settings](settings::Settings) for all options and [default.toml](https://github.com/dan-online/autopulse/blob/main/default.toml) for defaults
pub mod settings;

/// Arguments for CLI
///
/// ```
/// $ autopulse --help
/// ```
///
/// See [Args](cli::Args) for all options
pub mod cli;
