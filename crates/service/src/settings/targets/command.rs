use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use autopulse_database::models::ScanEvent;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

/// Command target
///
/// Note: Either `path` or `raw` must be set but not both
#[derive(Serialize, Clone, Deserialize)]
pub struct Command {
    /// Path to the command to run
    ///
    /// Example: `/path/to/script.sh`
    pub path: Option<String>,
    /// Timeout for the command in seconds (default: 10)
    ///
    /// Example: `5`
    pub timeout: Option<u64>,
    /// Raw command to run
    ///
    /// Example: `echo $FILE_PATH >> list.log`
    pub raw: Option<String>,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
    /// Path filter matched against the target-rewritten path.
    #[serde(default)]
    pub filter: PathFilter,
}

impl Command {
    pub async fn run(&self, ev: &ScanEvent) -> anyhow::Result<()> {
        if self.path.is_some() && self.raw.is_some() {
            return Err(anyhow::anyhow!("command cannot have both path and raw"));
        }

        if self.path.is_none() && self.raw.is_none() {
            return Err(anyhow::anyhow!("command requires either path or raw"));
        }

        let ev_path = ev.get_path(&self.rewrite);

        if let Some(path) = self.path.clone() {
            let output = tokio::process::Command::new(path.clone())
                .arg(&ev_path)
                .output();

            let timeout = self.timeout.unwrap_or(10);

            let output = tokio::time::timeout(std::time::Duration::from_secs(timeout), output)
                .await
                .map_err(|_| anyhow::anyhow!("command timed out"))??;

            debug!("command output: {:?}", output);

            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "command failed with status: {}",
                    output.status
                ));
            }
        }

        if let Some(raw) = self.raw.clone() {
            let output = tokio::process::Command::new("sh")
                .env("FILE_PATH", &ev_path)
                .arg("-c")
                .arg(&raw)
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

        Ok(())
    }
}

impl TargetProcess for Command {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeeded = Vec::new();

        for ev in evs {
            if let Err(e) = self.run(ev).await {
                error!("failed to process '{}': {}", ev.get_path(&self.rewrite), e);
            } else {
                succeeded.push(ev.id.clone());
            }
        }

        Ok(succeeded)
    }
}

#[cfg(test)]
mod tests {
    use super::Command;
    use autopulse_database::models::ScanEvent;

    fn scan_event() -> ScanEvent {
        let now = chrono::Utc::now().naive_utc();

        ScanEvent {
            id: "event-id".to_string(),
            event_source: "manual".to_string(),
            event_timestamp: now,
            file_path: "/media/movie.mkv".to_string(),
            file_hash: None,
            process_status: "pending".to_string(),
            found_status: "found".to_string(),
            failed_times: 0,
            next_retry_at: None,
            targets_hit: String::new(),
            found_at: None,
            processed_at: None,
            created_at: now,
            updated_at: now,
            can_process: now,
        }
    }

    #[tokio::test]
    async fn run_rejects_command_without_path_or_raw() {
        let command = Command {
            path: None,
            timeout: None,
            raw: None,
            rewrite: None,
            filter: Default::default(),
        };

        let err = command
            .run(&scan_event())
            .await
            .expect_err("empty command target should fail");

        assert!(
            err.to_string()
                .contains("command requires either path or raw"),
            "unexpected error: {err}"
        );
    }
}
