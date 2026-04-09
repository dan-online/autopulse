/// Autoscan - autoscan compatibility trigger
///
/// Provides a trigger with compatibility for applications that are built for autoscan
///
/// # Example
///
/// ```yml
/// triggers:
///   my_autoscan:
///     type: autoscan
/// ```
///
/// or
///
/// ```yml
/// triggers:
///   my_autoscan:
///     type: autoscan
///     rewrite:
///       from: "/downloads/all"
///       to: "/all"
///     timer:
///       wait: 30
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Autoscan`] for all options
pub mod autoscan;
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Lidarr`] for all options
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Manual`] for all options
/// and
/// [`ManualQueryParams`](manual::ManualQueryParams) for query parameters
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Notify`] for all options
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Radarr`] for all options
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Readarr`] for all options
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
///     excludes: [ "ignored_target" ]
/// ```
///
/// See [`Sonarr`] for all options
pub mod sonarr;

use crate::settings::timer::EventTimers;
use crate::settings::timer::Timer;
use crate::settings::{rewrite::Rewrite, triggers::autoscan::Autoscan};
use serde::{Deserialize, Serialize};
use {
    lidarr::{Lidarr, LidarrRequest},
    manual::Manual,
    notify::Notify,
    radarr::{Radarr, RadarrRequest},
    readarr::{Readarr, ReadarrRequest},
    sonarr::{Sonarr, SonarrRequest},
};

pub trait TriggerRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self>
    where
        Self: Sized;

    // where the bool represents whether to check found status
    fn paths(&self) -> Vec<(String, bool)>;
}

pub trait TriggerConfig {
    fn rewrite(&self) -> Option<&Rewrite>;
    fn timer(&self) -> Option<&Timer>;
    fn excludes(&self) -> &Vec<String>;
    fn event_timers(&self) -> Option<&EventTimers> {
        None
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerType {
    Manual,
    Autoscan,
    Radarr,
    Bazarr,
    Sonarr,
    Lidarr,
    Readarr,
    Notify,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Trigger {
    Manual(Manual),
    Autoscan(Autoscan),
    Bazarr(Manual),
    Radarr(Radarr),
    Sonarr(Sonarr),
    Lidarr(Lidarr),
    Readarr(Readarr),
    Notify(Notify),
}

impl Trigger {
    fn as_config(&self) -> &dyn TriggerConfig {
        match self {
            Self::Manual(trigger) | Self::Bazarr(trigger) => trigger,
            Self::Autoscan(trigger) => trigger,
            Self::Radarr(trigger) => trigger,
            Self::Sonarr(trigger) => trigger,
            Self::Lidarr(trigger) => trigger,
            Self::Readarr(trigger) => trigger,
            Self::Notify(trigger) => trigger,
        }
    }

    pub fn get_rewrite(&self) -> Option<&Rewrite> {
        self.as_config().rewrite()
    }

    pub fn get_timer(&self, event_name: Option<String>) -> Timer {
        let config = self.as_config();
        let mut base_timer = config.timer().cloned().unwrap_or_default();

        let event_specific_timer = event_name
            .as_ref()
            .and_then(|event| config.event_timers().and_then(|timers| timers.get(event)));

        if let Some(event_timer) = event_specific_timer {
            base_timer = base_timer.chain(event_timer);
        }

        base_timer
    }

    pub fn paths(&self, body: serde_json::Value) -> anyhow::Result<(String, Vec<(String, bool)>)> {
        let event_name = body["eventType"].as_str().unwrap_or("unknown").to_string();

        let paths = match &self {
            Self::Sonarr(_) => Ok(SonarrRequest::from_json(body)?.paths()),
            Self::Radarr(_) => Ok(RadarrRequest::from_json(body)?.paths()),
            Self::Lidarr(_) => Ok(LidarrRequest::from_json(body)?.paths()),
            Self::Readarr(_) => Ok(ReadarrRequest::from_json(body)?.paths()),
            Self::Manual(_) | Self::Notify(_) | Self::Autoscan(_) | Self::Bazarr(_) => {
                Err(anyhow::anyhow!("Manual trigger does not have paths"))
            }
        }?;

        Ok((event_name, paths))
    }

    pub fn excludes(&self) -> &Vec<String> {
        self.as_config().excludes()
    }
}
