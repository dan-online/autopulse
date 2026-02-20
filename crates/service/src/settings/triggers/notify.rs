use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use autopulse_utils::regex::Regex;
use notify_debouncer_full::{
    new_debouncer, new_debouncer_opt,
    notify::{
        event::{AccessKind, AccessMode, ModifyKind, RenameMode},
        Config, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode,
    },
    DebounceEventResult, Debouncer, NoCache, RecommendedCache,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing::{error, trace};

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug, Default)]
pub enum NotifyBackendType {
    #[serde(rename = "recommended")]
    /// Uses the recommended backend such as `inotify` on Linux, `FSEvents` on macOS, and `ReadDirectoryChangesW` on Windows
    #[default]
    Recommended,
    #[serde(rename = "polling")]
    /// Uses a polling backend (useful for rclone/nfs/etc mounts), which will be extremely inefficient with a high number of files
    Polling,
}

#[doc(hidden)]
pub enum NotifyBackend {
    Recommended(Debouncer<RecommendedWatcher, RecommendedCache>),
    Polling(Debouncer<PollWatcher, NoCache>),
}

impl NotifyBackend {
    pub fn watch(&mut self, path: String, mode: RecursiveMode) -> anyhow::Result<()> {
        let path = std::path::Path::new(&path);

        match self {
            Self::Recommended(debouncer) => debouncer.watch(path, mode).map_err(Into::into),
            Self::Polling(debouncer) => debouncer.watch(path, mode).map_err(Into::into),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Notify {
    /// Paths to monitor
    pub paths: Vec<String>,
    /// Rewrite path
    pub rewrite: Option<Rewrite>,
    /// Recursive monitoring (default: true)
    pub recursive: Option<bool>,
    /// Backend to use
    /// - `recommended`: Uses the recommended backend such as `inotify` on Linux, `FSEvents` on macOS, and `ReadDirectoryChangesW` on Windows
    /// - `polling`: Uses a polling backend (useful for rclone/nfs/etc mounts), which will be extremely inefficient with a high number of files
    #[serde(default)]
    pub backend: NotifyBackendType,
    /// Filter by regex
    pub filters: Option<Vec<String>>,

    /// Targets to exclude
    #[serde(default)]
    pub excludes: Vec<String>,
    /// Timer
    pub timer: Option<Timer>,
    /// Debounce timeout in seconds (default: 2)
    pub debounce: Option<u64>,
}

impl Notify {
    pub fn send_event(
        &self,
        tx: UnboundedSender<(String, EventKind)>,
        path: Option<&PathBuf>,
        reason: EventKind,
    ) -> anyhow::Result<()> {
        if path.is_none() {
            return Ok(());
        }

        let mut path = path.unwrap().to_string_lossy().to_string();

        if let Some(ref filters) = self.filters {
            let mut matched = false;

            for regex in filters {
                if Regex::new(regex)?.is_match(&path) {
                    matched = true;
                    break;
                }
            }

            if !matched {
                return Ok(());
            }
        }

        if let Some(rewrite) = &self.rewrite {
            path = rewrite.rewrite_path(path);
        }

        tx.send((path, reason)).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn async_watcher(
        &self,
    ) -> anyhow::Result<(NotifyBackend, UnboundedReceiver<DebounceEventResult>)> {
        let (tx, rx) = unbounded_channel();

        let event_handler = move |result: DebounceEventResult| {
            if let Err(e) = tx.send(result) {
                error!("failed to process notify event: {e}");
            }
        };

        let timeout = Duration::from_secs(self.debounce.unwrap_or(2));

        if self.backend == NotifyBackendType::Recommended {
            let debouncer = new_debouncer(timeout, None, event_handler)?;

            Ok((NotifyBackend::Recommended(debouncer), rx))
        } else {
            let debouncer = new_debouncer_opt::<_, PollWatcher, NoCache>(
                timeout,
                None,
                event_handler,
                NoCache,
                Config::default().with_poll_interval(Duration::from_secs(10)),
            )?;

            Ok((NotifyBackend::Polling(debouncer), rx))
        }
    }

    pub async fn watcher(&self, tx: UnboundedSender<(String, EventKind)>) -> anyhow::Result<()> {
        let (mut watcher, mut rx) = self.async_watcher()?;

        for path in &self.paths {
            let start = std::time::Instant::now();

            let recursive_mode = self.recursive.unwrap_or(true);

            watcher.watch(
                path.clone(),
                if recursive_mode {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )?;

            if let NotifyBackend::Polling(_) = watcher {
                trace!("watching '{}' took: {:?}", path, start.elapsed());
            }
        }

        while let Some(result) = rx.recv().await {
            match result {
                Ok(events) => {
                    for debounced_event in events {
                        let kind = debounced_event.event.kind;

                        match kind {
                            EventKind::Access(AccessKind::Close(AccessMode::Write))
                            | EventKind::Modify(
                                ModifyKind::Metadata(_) | ModifyKind::Name(RenameMode::Both),
                            )
                            | EventKind::Create(_)
                            | EventKind::Remove(_) => {
                                for path in debounced_event.event.paths {
                                    self.send_event(tx.clone(), Some(&path), kind)?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        error!("failed to process notify event: {error}");
                    }
                }
            }
        }

        Ok(())
    }
}
