use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use crate::settings::triggers::TriggerRequest;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct ATrain {
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Timer settings
    #[serde(default)]
    pub timer: Timer,
    /// Targets to ignore
    #[serde(default)]
    pub excludes: Vec<String>,
}

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
        let mut paths = vec![];
        for path in &self.created {
            paths.push((path.clone(), true));
        }
        for path in &self.deleted {
            paths.push((path.clone(), false));
        }
        paths
    }
}
