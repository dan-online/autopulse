use std::sync::Arc;
use tokio::{
    sync::{Mutex, Notify},
    task::JoinHandle,
};

pub struct TaskManager {
    tasks: Arc<Mutex<Vec<Arc<JoinHandle<()>>>>>,
    shutdown: Arc<Notify>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Spawns a new task and tracks it
    pub async fn spawn<F, T>(&self, fut: F) -> Arc<JoinHandle<()>>
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let shutdown = self.shutdown.clone();
        let fut = async move {
            tokio::select! {
                _ = fut => {},
                _ = shutdown.notified() => {
                    // Task was asked to shut down
                },
            }
        };

        let handle: JoinHandle<()> = tokio::spawn(fut);
        let handle = Arc::new(handle);

        self.tasks.lock().await.push(handle.clone());

        handle
    }

    /// Signals all tasks to shut down
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.shutdown.notify_waiters();

        let tasks = self.tasks.lock().await.drain(..).collect::<Vec<_>>();
        for handle in tasks {
            handle.abort();
        }

        Ok(())
    }
}
