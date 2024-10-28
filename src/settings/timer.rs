use serde::Deserialize;

/// Timer structure for triggers
///
/// Example:
///
/// ```yml
/// triggers:
///  sonarr:
///   type: sonarr
///   timer:
///    wait: 300 # wait 5 minutes before processing
/// ```
#[derive(Deserialize, Clone, Default)]
pub struct Timer {
    /// Time to wait before processing
    pub wait: Option<u64>,
}
