use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use crate::settings::triggers::{TriggerConfig, TriggerRequest};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ATrain {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    pub timer: Option<Timer>,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
    /// Path filter matched against the rewritten file path.
    #[serde(default)]
    pub filter: PathFilter,
}

impl TriggerConfig for ATrain {
    fn rewrite(&self) -> Option<&Rewrite> {
        self.rewrite.as_ref()
    }

    fn timer(&self) -> Option<&Timer> {
        self.timer.as_ref()
    }

    fn excludes(&self) -> &Vec<String> {
        &self.excludes
    }

    fn filter(&self) -> &PathFilter {
        &self.filter
    }

    fn accepts_trailing_segment(&self) -> bool {
        true
    }
}

/// Payload sent by A-Train on every change set.
///
/// A-Train POSTs this body to `/triggers/a-train/{drive_id}`. Each entry is a
/// directory path (A-Train trims file leaves and reports parents) on the
/// service-account-mounted Google Drive. `created` entries are treated as
/// `NotFound` so autopulse will verify them on disk before dispatching;
/// `deleted` entries skip verification.
#[derive(Deserialize, Clone)]
#[doc(hidden)]
pub struct ATrainRequest {
    #[serde(default)]
    pub created: Vec<String>,
    #[serde(default)]
    pub deleted: Vec<String>,
}

impl TriggerRequest for ATrainRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }

    fn paths(&self) -> Vec<(String, bool)> {
        // Upstream A-Train coalesces parents into a `HashSet<PathBuf>` before
        // sending, but a fork or replayed payload could still ship duplicates.
        // Dedup on (path, search) while preserving insertion order so created
        // events stay before deleted events for the same path.
        let mut seen = std::collections::HashSet::new();
        let mut paths = Vec::with_capacity(self.created.len() + self.deleted.len());
        for path in &self.created {
            let entry = (path.clone(), true);
            if seen.insert(entry.clone()) {
                paths.push(entry);
            }
        }
        for path in &self.deleted {
            let entry = (path.clone(), false);
            if seen.insert(entry.clone()) {
                paths.push(entry);
            }
        }
        paths
    }
}
