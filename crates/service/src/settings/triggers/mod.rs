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

use crate::settings::timer::Timer;
use crate::settings::{rewrite::Rewrite, triggers::autoscan::Autoscan};
use serde::Deserialize;
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

#[derive(Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Trigger {
    Manual(Manual),
    Autoscan(Autoscan),
    Radarr(Radarr),
    Sonarr(Sonarr),
    Lidarr(Lidarr),
    Readarr(Readarr),
    Notify(Notify),
}

impl Trigger {
    pub const fn get_rewrite(&self) -> Option<&Rewrite> {
        match &self {
            Self::Sonarr(trigger) => trigger.rewrite.as_ref(),
            Self::Radarr(trigger) => trigger.rewrite.as_ref(),
            Self::Lidarr(trigger) => trigger.rewrite.as_ref(),
            Self::Readarr(trigger) => trigger.rewrite.as_ref(),
            Self::Autoscan(trigger) => trigger.rewrite.as_ref(),
            Self::Manual(trigger) => trigger.rewrite.as_ref(),
            Self::Notify(trigger) => trigger.rewrite.as_ref(),
        }
    }

    pub fn get_timer(&self, event_name: Option<String>) -> Timer {
        let mut base_timer = match self.clone() {
            Self::Sonarr(trigger) => trigger.timer,
            Self::Radarr(trigger) => trigger.timer,
            Self::Lidarr(trigger) => trigger.timer,
            Self::Readarr(trigger) => trigger.timer,
            Self::Manual(trigger) => trigger.timer,
            Self::Notify(trigger) => trigger.timer,
            Self::Autoscan(trigger) => trigger.timer,
        };

        let event_specific_timer = match &self {
            Self::Sonarr(trigger) => event_name
                .as_ref()
                .and_then(|event| trigger.event_timers.get(event)),
            Self::Radarr(trigger) => event_name
                .as_ref()
                .and_then(|event| trigger.event_timers.get(event)),
            Self::Lidarr(trigger) => event_name
                .as_ref()
                .and_then(|event| trigger.event_timers.get(event)),
            Self::Readarr(trigger) => event_name
                .as_ref()
                .and_then(|event| trigger.event_timers.get(event)),
            _ => None,
        };

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
            Self::Manual(_) | Self::Notify(_) | Self::Autoscan(_) => {
                Err(anyhow::anyhow!("Manual trigger does not have paths"))
            }
        }?;

        Ok((event_name, paths))
    }

    pub const fn excludes(&self) -> &Vec<String> {
        match &self {
            Self::Manual(trigger) => &trigger.excludes,
            Self::Radarr(trigger) => &trigger.excludes,
            Self::Sonarr(trigger) => &trigger.excludes,
            Self::Lidarr(trigger) => &trigger.excludes,
            Self::Readarr(trigger) => &trigger.excludes,
            Self::Notify(trigger) => &trigger.excludes,
            Self::Autoscan(trigger) => &trigger.excludes,
        }
    }
}
