use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Clone, Default)]
pub struct Timer {
    /// Time to wait before processing
    pub wait: Option<u64>,
}

impl Timer {
    pub fn chain(&self, link: &Self) -> Self {
        Self {
            wait: self
                .wait
                .or(link.wait)
                .map(|wait| wait + link.wait.unwrap_or(0)),
        }
    }
}

/// Define timers that apply to only specific events
///
/// -rr events:
/// - `Download` - when a download is completed
/// - `Rename` - when a file is renamed
///
/// Lidarr:
/// - `ArtistDelete` - when an artist is deleted
///
/// Radarr:
/// - `MovieDelete` - when a movie is deleted
/// - `MovieFileDelete` - when a movie file is deleted
///
/// Readarr:
/// - `AuthorDelete` - when an author is deleted
/// - `BookDelete` - when a book is deleted
/// - `BookFileDelete` - when a book file is deleted
///
/// Sonarr:
/// - `SeriesDelete` - when a series is deleted
/// - `EpisodeFileDelete` - when an episode file is deleted
///
/// **Note: These timers apply on top of the original timer**
#[doc(hidden)]
#[derive(Clone)]
pub struct EventTimers {
    timers: HashMap<String, Timer>,
}

impl EventTimers {
    pub fn get(&self, event_name: &str) -> Option<&Timer> {
        self.timers.get(event_name.to_lowercase().as_str())
    }
}

impl<'de> Deserialize<'de> for EventTimers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map: HashMap<String, Timer> = HashMap::deserialize(deserializer)?;

        Ok(Self {
            timers: map
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v))
                .collect(),
        })
    }
}
