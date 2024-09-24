use crate::{db::models::ScanEvent, utils::settings::TargetProcess};
use serde::Deserialize;
use tracing::{debug, error};

/// Command target
///
/// Note: Either `path` or `raw` must be set but not both
#[derive(Clone, Deserialize)]
pub struct Command {
    /// Path to the command to run
    ///
    /// Example: `/path/to/script.sh`
    path: Option<String>,
    /// Timeout for the command in seconds (default: 10)
    ///
    /// Example: `5`
    timeout: Option<u64>,
    /// Raw command to run
    ///
    /// Example: `echo $FILE_PATH >> list.log`
    raw: Option<String>,
}

impl Command {
    pub async fn run(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        if self.path.is_some() && self.raw.is_some() {
            return Err(anyhow::anyhow!("command cannot have both path and raw"));
        }

        if let Some(path) = self.path.clone() {
            let output = tokio::process::Command::new(path.clone())
                .arg(&ev.file_path)
                .output();

            let timeout = self.timeout.unwrap_or(10);

            let output = tokio::time::timeout(std::time::Duration::from_secs(timeout), output)
                .await
                .map_err(|_| anyhow::anyhow!("command timed out"))??;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "command failed with status: {}",
                    output.status
                ));
            }

            debug!("command output: {:?}", output);
        }

        if let Some(raw) = self.raw.clone() {
            let output = tokio::process::Command::new("sh")
                .env("FILE_PATH", &ev.file_path)
                .arg("-c")
                .arg(&raw)
                .output();

            let timeout = self.timeout.unwrap_or(10000);

            let output = tokio::time::timeout(std::time::Duration::from_millis(timeout), output)
                .await
                .map_err(|_| anyhow::anyhow!("command timed out"))??;

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "command failed with status: {}",
                    output.status
                ));
            }

            debug!("command output: {:?}", output);
        }

        Ok(())
    }
}

impl TargetProcess for Command {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeded = Vec::new();

        for ev in evs {
            if let Err(e) = self.run(ev).await {
                error!("failed to process '{}': {}", ev.file_path, e);
            } else {
                succeded.push(ev.id.clone());
            }
        }

        Ok(succeded)
    }
}
