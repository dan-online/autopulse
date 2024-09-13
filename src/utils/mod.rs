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
pub mod rewrite;

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
/// Timer to buffer events
///
/// Used to buffer events and process them in a batch
///
/// # Example
///
/// ```yml
/// triggers:
///   buffered:
///     type: manual
///     timer:
///       wait: 30
/// ```
///
/// Every time an event is triggered, it will reset the timer.
/// Once the timer reaches 30 seconds, it will process all of the events.
///
/// See [Timer](timer::Timer) for all options
pub mod timer;
