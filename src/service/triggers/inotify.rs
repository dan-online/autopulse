use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use inotify::{EventMask, Inotify, WatchMask};
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::utils::settings::Rewrite;

#[derive(Debug, Deserialize, Clone)]
pub struct InotifyService {
    pub paths: Vec<String>,
    pub rewrite: Option<Rewrite>,
    pub recursive: Option<bool>,
    pub exclude: Option<Vec<String>>,
}

impl InotifyService {
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
            let from = rewrite.from.clone();
            let to = rewrite.to.clone();

            path = path.replace(&from, &to);
        }

        tx.send(path).map_err(|e| anyhow::anyhow!(e))
    }

    pub fn watch_directory(&self, path: &Path, tx: UnboundedSender<String>) -> anyhow::Result<()> {
        let mut inotify = Inotify::init().expect("Failed to initialize inotify");

        inotify.watches().add(
            path,
            WatchMask::CREATE | WatchMask::DELETE | WatchMask::MODIFY | WatchMask::MOVE,
        )?;

        let mut buffer = [0u8; 4096];
        loop {
            let events = inotify.read_events_blocking(&mut buffer)?;

            for event in events {
                let event_path = match event.name {
                    Some(name) => path.join(name),
                    None => continue,
                };

                if event.mask.contains(EventMask::CREATE) {
                    if event.mask.contains(EventMask::ISDIR) {
                        if self.recursive.unwrap_or(false) {
                            // Initialize a new inotify instance for the new directory
                            let me = Arc::new(self.clone());

                            let tx = tx.clone();

                            tokio::spawn(async move {
                                me.watch_directory(&event_path, tx).unwrap();
                            });
                        }
                    } else {
                        self.send_event(tx.clone(), Some(&event_path))?;
                    }
                } else if event.mask.contains(EventMask::DELETE) {
                    self.send_event(tx.clone(), Some(&event_path))?;
                } else if event.mask.contains(EventMask::MODIFY) {
                    if !event.mask.contains(EventMask::ISDIR) {
                        self.send_event(tx.clone(), Some(&event_path))?;
                    }
                } else if event.mask.contains(EventMask::MOVED_FROM) {
                    self.send_event(tx.clone(), Some(&event_path))?;
                } else if event.mask.contains(EventMask::MOVED_TO) {
                    self.send_event(tx.clone(), Some(&event_path))?;
                }
            }
        }
    }

    pub fn watcher(&self, tx: UnboundedSender<String>) -> anyhow::Result<()> {
        for path in &self.paths {
            let walker = walkdir::WalkDir::new(path);
            for entry in walker {
                let entry = entry?;
                if entry.file_type().is_dir() {
                    let me = Arc::new(self.clone());
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        me.watch_directory(entry.path(), tx).unwrap();
                    });
                }
            }
        }

        Ok(())
    }
}
