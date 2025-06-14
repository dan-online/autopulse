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

/// Event timers
///
/// Define timers that apply to only specific events
///
/// Note: Keys are case insensitive
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
///
/// Example:
/// ```yaml
/// event_timers:
///   download:
///     wait: 10
///   
///   seriesdelete:
///     wait: 5
///
///   EpisodeFileDelete:
///     wait: 2
/// ```
#[derive(Clone, Default)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_chain() {
        let timer1 = Timer { wait: Some(10) };
        let timer2 = Timer { wait: Some(5) };
        let chained_timer = timer1.chain(&timer2);

        assert_eq!(chained_timer.wait, Some(15));
    }

    #[test]
    fn test_event_timers_get() {
        let mut event_timers = EventTimers {
            timers: HashMap::new(),
        };

        event_timers
            .timers
            .insert("download".to_string(), Timer { wait: Some(10) });

        assert_eq!(event_timers.get("download").unwrap().wait, Some(10));
        assert!(event_timers.get("unknown").is_none());
    }
}
