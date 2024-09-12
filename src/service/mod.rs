#[doc(hidden)]
pub mod manager;
#[doc(hidden)]
pub mod runner;

/// Library that will be updated when a file is ready to be processed
pub mod targets;
/// Endpoint that will be called when a file needs to be processed
pub mod triggers;
/// Webhooks that will be notified of events
pub mod webhooks;
