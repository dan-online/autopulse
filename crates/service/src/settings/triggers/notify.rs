use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use autopulse_utils::regex::Regex;
use notify::{
    event::{ModifyKind, RenameMode},
    Config, Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing::{error, trace};

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Debug)]
pub enum NotifyBackendType {
    #[serde(rename = "recommended")]
    /// Uses the recommended backend such as `inotify` on Linux, `FSEvents` on macOS, and `ReadDirectoryChangesW` on Windows
    Recommended,
    #[serde(rename = "polling")]
    /// Uses a polling backend (useful for rclone/nfs/etc mounts), which will be extremely inefficient with a high number of files
    Polling,
}

#[doc(hidden)]
pub enum NotifyBackend {
    Recommended(RecommendedWatcher),
    Polling(PollWatcher),
}

impl NotifyBackend {
    pub fn watch(&mut self, path: String, mode: RecursiveMode) -> anyhow::Result<()> {
        match self {
            Self::Recommended(watcher) => watcher.watch(path.as_ref(), mode).map_err(Into::into),
            Self::Polling(watcher) => watcher.watch(path.as_ref(), mode).map_err(Into::into),
        }
    }
}

impl Default for NotifyBackendType {
    fn default() -> Self {
        Self::Recommended
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
    ) -> anyhow::Result<(NotifyBackend, UnboundedReceiver<notify::Result<Event>>)> {
        let (tx, rx) = unbounded_channel();

        let event_handler = move |res| {
            if let Err(e) = tx.send(res) {
                error!("failed to process notify event: {e}");
            }
        };

        if self.backend == NotifyBackendType::Recommended {
            let watcher = RecommendedWatcher::new(event_handler, Config::default())?;

            Ok((NotifyBackend::Recommended(watcher), rx))
        } else {
            let watcher = PollWatcher::new(
                event_handler,
                Config::default().with_poll_interval(Duration::from_secs(10)),
            )?;

            // watcher.poll()?;

            Ok((NotifyBackend::Polling(watcher), rx))
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

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => match event.kind {
                    EventKind::Modify(
                        ModifyKind::Data(_)
                        | ModifyKind::Metadata(_)
                        | ModifyKind::Name(RenameMode::Both),
                    )
                    | EventKind::Create(_)
                    | EventKind::Remove(_) => {
                        for path in event.paths {
                            self.send_event(tx.clone(), Some(&path), event.kind)?;
                        }
                    }
                    _ => {}
                },
                Err(e) => error!("failed to process notify event: {e}"),
            }
        }

        Ok(())
    }
}
