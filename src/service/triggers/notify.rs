use crate::utils::{rewrite::rewrite_path, settings::Rewrite};
use notify::{
    event::{ModifyKind, RenameMode},
    Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tracing::error;

#[derive(Debug, Deserialize, Clone)]
pub struct NotifyService {
    pub paths: Vec<String>,
    pub rewrite: Option<Rewrite>,
    pub recursive: Option<bool>,
    // pub exclude: Option<Vec<String>>,
}

impl NotifyService {
    pub fn send_event(
        &self,
        tx: UnboundedSender<String>,
        path: Option<&PathBuf>,
    ) -> anyhow::Result<()> {
        if path.is_none() {
            return Ok(());
        }

        let mut path = path.unwrap().to_string_lossy().to_string();

        if let Some(rewrite) = &self.rewrite {
            path = rewrite_path(path, rewrite);
        }

        tx.send(path).map_err(|e| anyhow::anyhow!(e))
    }

    fn async_watcher(
        &self,
    ) -> notify::Result<(RecommendedWatcher, UnboundedReceiver<notify::Result<Event>>)> {
        let (tx, rx) = unbounded_channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                if let Err(e) = tx.send(res) {
                    error!("unable to process notify event: {e}")
                }
            },
            Config::default(),
        )?;

        Ok((watcher, rx))
    }

    pub async fn watcher(&self, tx: UnboundedSender<String>) -> anyhow::Result<()> {
        let (mut watcher, mut rx) = self.async_watcher()?;

        for path in &self.paths {
            watcher.watch(
                path.as_ref(),
                if self.recursive.unwrap_or(true) {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )?;
        }

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => match event.kind {
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Metadata(_))
                    | EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                    | EventKind::Create(_)
                    | EventKind::Remove(_) => {
                        for path in event.paths {
                            self.send_event(tx.clone(), Some(&path))?;
                        }
                    }
                    _ => {}
                },
                Err(e) => error!("unable to process notify event: {e}"),
            }
        }

        Ok(())
    }
}
