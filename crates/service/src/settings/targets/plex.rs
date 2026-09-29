use super::{Request, RequestBuilderPerform};
use crate::settings::path_filter::PathFilter;
use crate::settings::rewrite::Rewrite;
use crate::settings::targets::TargetProcess;
use anyhow::Context;
use autopulse_database::models::ScanEvent;
use autopulse_utils::{get_url, RuntimePath};
use futures::StreamExt;
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, trace, warn};

#[derive(Serialize, Deserialize, Clone)]
pub struct Plex {
    /// URL to the Plex server
    pub url: String,
    /// API token for the Plex server
    pub token: String,
    /// Whether to refresh metadata of the file (default: false)
    #[serde(default)]
    pub refresh: bool,
    /// Whether to analyze the file (default: false)
    #[serde(default)]
    pub analyze: bool,
    /// Empty library trash when Plex reports scan completion (default: false).
    /// Removes all unavailable items in each scanned library, not just the scanned paths.
    #[serde(default)]
    pub empty_trash: bool,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
    /// Path filter matched against the target-rewritten path.
    #[serde(default)]
    pub filter: PathFilter,
    /// HTTP request options
    #[serde(default)]
    pub request: Request,
    #[serde(skip)]
    runtime: Arc<PlexRuntime>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    #[serde(rename = "Part")]
    pub part: Vec<Part>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    // pub id: i64,
    pub key: String,
    // pub duration: Option<i64>,
    pub file: String,
    // pub size: i64,
    // pub audio_profile: Option<String>,
    // pub container: Option<String>,
    // pub video_profile: Option<String>,
    // pub has_thumbnail: Option<String>,
    // pub has64bit_offsets: Option<bool>,
    // pub optimized_for_streaming: Option<bool>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub key: String,
    #[serde(rename = "Media")]
    pub media: Option<Vec<Media>>,
    #[serde(rename = "type")]
    pub t: String,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Location {
    path: String,
}

#[doc(hidden)]
#[derive(Deserialize, Clone, Debug)]
struct Library {
    title: String,
    key: String,
    #[serde(rename = "Location")]
    location: Vec<Location>,
}

struct LibraryCleanup {
    scan_failed: bool,
}

#[derive(Default)]
struct PlexRuntime {
    libraries: Mutex<Vec<Library>>,
    generations: Mutex<HashMap<String, u64>>,
    cleanups: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

#[derive(Deserialize, Debug)]
struct Notification {
    #[serde(rename = "NotificationContainer")]
    container: NotificationContainer,
}

#[derive(Deserialize, Debug, Default)]
struct NotificationContainer {
    #[serde(rename = "ActivityNotification", default)]
    activities: Vec<ActivityNotification>,
}

#[derive(Deserialize, Debug)]
struct ActivityNotification {
    event: String,
    uuid: String,
    #[serde(rename = "Activity")]
    activity: Activity,
}

#[derive(Deserialize, Debug)]
struct Activity {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "Context")]
    context: Option<ActivityContext>,
}

#[derive(Deserialize, Debug)]
struct ActivityContext {
    #[serde(rename = "librarySectionID")]
    library_section_id: Option<String>,
}

#[derive(Default)]
struct ScanNotificationState {
    active: HashMap<String, Option<String>>,
    completed_sections: HashSet<String>,
    last_scan_notification: HashMap<String, tokio::time::Instant>,
}

impl ScanNotificationState {
    fn apply(&mut self, notification: Notification) {
        for notification in notification.container.activities {
            if notification.activity.kind != "library.update.section" {
                continue;
            }
            let section = notification
                .activity
                .context
                .and_then(|context| context.library_section_id)
                .or_else(|| self.active.get(&notification.uuid).cloned().flatten());

            match notification.event.as_str() {
                "started" | "updated" => {
                    if let Some(section) = &section {
                        self.last_scan_notification
                            .insert(section.clone(), tokio::time::Instant::now());
                    }
                    self.active.insert(notification.uuid, section);
                }
                "ended" => {
                    self.active.remove(&notification.uuid);
                    if let Some(section) = section {
                        self.last_scan_notification
                            .insert(section.clone(), tokio::time::Instant::now());
                        self.completed_sections.insert(section);
                    }
                }
                _ => {}
            }
        }
    }

    fn completion_confirmed(&self, section: &str) -> bool {
        self.completed_sections.contains(section)
            && !self.active.values().any(|active_section| {
                active_section
                    .as_deref()
                    .is_some_and(|active| active == section)
            })
    }
}

type PlexWebSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[cfg(not(test))]
const SCAN_QUIET_PERIOD: Duration = Duration::from_secs(5);
#[cfg(test)]
const SCAN_QUIET_PERIOD: Duration = Duration::from_millis(10);

struct ScanNotifications {
    receiver: tokio::sync::mpsc::UnboundedReceiver<NotificationStreamEvent>,
    state: ScanNotificationState,
    reader: tokio::task::JoinHandle<()>,
}

enum NotificationStreamEvent {
    Notification(Notification),
    Reconnected,
}

impl ScanNotifications {
    async fn connect(url: url::Url) -> anyhow::Result<Self> {
        let (socket, _) = connect_async(url.as_str())
            .await
            .context("failed to connect to Plex notification websocket")?;
        debug!("connected to Plex notification websocket");
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let reader = tokio::spawn(Self::read_notifications(url, socket, sender));
        Ok(Self {
            receiver,
            state: ScanNotificationState::default(),
            reader,
        })
    }

    async fn reconnect(url: &url::Url) -> PlexWebSocket {
        let mut delay = Duration::from_secs(1);
        loop {
            warn!(
                "Plex notification websocket disconnected; reconnecting in {} second(s)",
                delay.as_secs()
            );
            tokio::time::sleep(delay).await;
            match connect_async(url.as_str()).await {
                Ok((socket, _)) => {
                    debug!("reconnected to Plex notification websocket");
                    return socket;
                }
                Err(error) => {
                    warn!("failed to reconnect to Plex notification websocket: {error}");
                    delay = (delay * 2).min(Duration::from_secs(30));
                }
            }
        }
    }

    async fn read_notifications(
        url: url::Url,
        mut socket: PlexWebSocket,
        sender: tokio::sync::mpsc::UnboundedSender<NotificationStreamEvent>,
    ) {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(payload))) => {
                    match serde_json::from_str::<Notification>(&payload) {
                        Ok(notification) => {
                            if sender
                                .send(NotificationStreamEvent::Notification(notification))
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(error) => trace!("ignored malformed Plex notification: {error}"),
                    }
                }
                Some(Ok(Message::Binary(payload))) => {
                    match serde_json::from_slice::<Notification>(&payload) {
                        Ok(notification) => {
                            if sender
                                .send(NotificationStreamEvent::Notification(notification))
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(error) => trace!("ignored malformed Plex notification: {error}"),
                    }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                    socket = Self::reconnect(&url).await;
                    if sender.send(NotificationStreamEvent::Reconnected).is_err() {
                        return;
                    }
                }
                Some(Ok(_)) => {}
            }
        }
    }

    fn apply(&mut self, event: NotificationStreamEvent) {
        match event {
            NotificationStreamEvent::Notification(notification) => self.state.apply(notification),
            NotificationStreamEvent::Reconnected => {
                // Events may have been lost while disconnected. Require fresh completion
                // evidence per section instead of risking cleanup after a missed scan.
                self.state.active.clear();
                self.state.completed_sections.clear();
                self.state.last_scan_notification.clear();
            }
        }
    }

    fn drain_pending(&mut self) -> anyhow::Result<()> {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => self.apply(event),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return Ok(()),
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    anyhow::bail!("Plex notification websocket reader stopped")
                }
            }
        }
    }

    async fn wait_for_scan(&mut self, section: &str) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(300), async {
            loop {
                self.drain_pending()?;
                let quiet_until = self.state.completion_confirmed(section).then(|| {
                    self.state
                        .last_scan_notification
                        .get(section)
                        .copied()
                        .expect("a completed section has a notification timestamp")
                        + SCAN_QUIET_PERIOD
                });

                if let Some(deadline) = quiet_until {
                    if tokio::time::Instant::now() >= deadline {
                        // Give the always-running reader a chance to enqueue socket data that
                        // became readable at the same instant as the quiet-period timer.
                        tokio::task::yield_now().await;
                        self.drain_pending()?;
                        if self.state.completion_confirmed(section)
                            && self
                                .state
                                .last_scan_notification
                                .get(section)
                                .is_some_and(|last| {
                                    *last + SCAN_QUIET_PERIOD <= tokio::time::Instant::now()
                                })
                        {
                            return Ok::<_, anyhow::Error>(());
                        }
                        continue;
                    }

                    tokio::select! {
                        biased;
                        event = self.receiver.recv() => {
                            self.apply(event.context("Plex notification websocket reader stopped")?);
                        }
                        () = tokio::time::sleep_until(deadline) => {}
                    }
                } else {
                    let event = self
                        .receiver
                        .recv()
                        .await
                        .context("Plex notification websocket reader stopped")?;
                    self.apply(event);
                }
            }
        })
        .await
        .context("timed out waiting for Plex library scans to finish")?
    }
}

impl Drop for ScanNotifications {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct LibraryMediaContainer {
    directory: Option<Vec<Library>>,
    metadata: Option<Vec<Metadata>>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SearchResult {
    metadata: Option<Metadata>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SearchLibraryMediaContainer {
    #[serde(default)]
    search_result: Vec<SearchResult>,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct LibraryResponse {
    media_container: LibraryMediaContainer,
}

#[doc(hidden)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SearchLibraryResponse {
    media_container: SearchLibraryMediaContainer,
}

fn path_matches(part_file: &str, path: &str) -> bool {
    let part_file = RuntimePath::new(part_file);
    let path = RuntimePath::new(path);

    if path.is_directory() {
        part_file.starts_with(path)
    } else {
        part_file.equals(path)
    }
}

fn has_matching_media(media: &[Media], path: &str) -> bool {
    media.iter().any(|media_item| {
        media_item
            .part
            .iter()
            .any(|part| path_matches(&part.file, path))
    })
}

fn scan_directory(path: &str) -> &str {
    RuntimePath::new(path).parent_or_self().as_str()
}

impl Plex {
    pub(crate) fn queue_event(&self, ev: &ScanEvent) {
        if !self.empty_trash {
            return;
        }
        let path = ev.get_path(&self.rewrite);
        let libraries = self.runtime.libraries.lock().unwrap();
        let sections = self
            .get_libraries(&libraries, &path)
            .into_iter()
            .map(|library| library.key)
            .collect::<HashSet<_>>();
        drop(libraries);
        self.invalidate_cleanups(&sections);
    }

    fn invalidate_cleanups(&self, sections: &HashSet<String>) -> HashMap<String, u64> {
        let mut generations = self.runtime.generations.lock().unwrap();
        let mut current = HashMap::new();
        for section in sections {
            let generation = generations.entry(section.clone()).or_default();
            *generation = generation.wrapping_add(1);
            current.insert(section.clone(), *generation);
        }
        drop(generations);

        let mut cleanups = self.runtime.cleanups.lock().unwrap();
        for section in sections {
            if let Some(cleanup) = cleanups.remove(section) {
                cleanup.abort();
            }
        }
        current
    }

    fn cleanup_generation(&self, section: &str) -> u64 {
        self.runtime
            .generations
            .lock()
            .unwrap()
            .get(section)
            .copied()
            .unwrap_or_default()
    }

    fn schedule_cleanup(
        &self,
        section: String,
        generation: u64,
        mut notifications: ScanNotifications,
    ) {
        let plex = self.clone();
        let task_section = section.clone();
        let cleanup = tokio::spawn(async move {
            let result = notifications.wait_for_scan(&task_section).await;
            if plex.cleanup_generation(&task_section) != generation {
                debug!("cancelled stale trash cleanup for library '{task_section}'");
                return;
            }
            match result {
                Ok(()) => match plex.empty_library_trash(&task_section).await {
                    Ok(()) => debug!("emptied trash for library '{task_section}'"),
                    Err(error) => {
                        error!("failed to empty trash for library '{task_section}': {error:#}")
                    }
                },
                Err(error) => {
                    error!("failed to empty trash for library '{task_section}': {error:#}")
                }
            }
        });

        if let Some(previous) = self
            .runtime
            .cleanups
            .lock()
            .unwrap()
            .insert(section, cleanup)
        {
            previous.abort();
        }
    }

    fn get_client(&self) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        headers.insert("X-Plex-Token", self.token.parse()?);
        headers.insert("Accept", "application/json".parse()?);

        self.request
            .client_builder(headers)
            .build()
            .map_err(Into::into)
    }

    fn notification_url(&self) -> anyhow::Result<url::Url> {
        let mut url = get_url(&self.url)?.join(":/websockets/notifications")?;
        match url.scheme() {
            "http" => url.set_scheme("ws").expect("ws is a valid URL scheme"),
            "https" => url.set_scheme("wss").expect("wss is a valid URL scheme"),
            scheme => anyhow::bail!("unsupported Plex URL scheme '{scheme}'"),
        }
        url.query_pairs_mut()
            .append_pair("X-Plex-Token", &self.token);
        Ok(url)
    }

    async fn libraries(&self) -> anyhow::Result<Vec<Library>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join("library/sections")?;

        let res = client.get(url).perform().await?;

        let libraries: LibraryResponse = res.json().await?;

        Ok(libraries.media_container.directory.unwrap_or_default())
    }

    fn get_libraries(&self, libraries: &[Library], path: &str) -> Vec<Library> {
        let event_path = RuntimePath::new(path);
        let mut matches: Vec<(usize, &Library)> = vec![];

        for library in libraries {
            for location in &library.location {
                let location_path = RuntimePath::new(&location.path);
                if event_path.starts_with(location_path) {
                    matches.push((location_path.component_count(), library));
                }
            }
        }

        // Most-specific (highest component count) match first
        matches.sort_by(|(components_a, _), (components_b, _)| components_b.cmp(components_a));

        matches
            .into_iter()
            .map(|(_, library)| library.clone())
            .collect()
    }

    async fn get_episodes(&self, key: &str) -> anyhow::Result<LibraryResponse> {
        let client = self.get_client()?;

        // remove last part of the key
        let key = key.rsplit_once('/').map(|x| x.0).unwrap_or(key);

        let url = get_url(&self.url)?.join(&format!("{key}/allLeaves"))?;

        let res = client.get(url).perform().await?;

        let lib: LibraryResponse = res.json().await?;

        Ok(lib)
    }

    fn get_search_term(&self, path: &str) -> anyhow::Result<String> {
        let parent_or_directory = RuntimePath::new(path).parent_or_self();
        let components = parent_or_directory.normal_components().collect::<Vec<_>>();

        let chosen = components
            .iter()
            .rev()
            .copied()
            .find(|component| !component.contains("Season") && !component.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                // All components were "Season N": use normalized components,
                // not raw source, to keep drive letters/UNC/backslashes out
                components.join(" ")
            });

        Ok(chosen
            .split_whitespace()
            .filter(|part| {
                ["(", ")", "[", "]", "{", "}"]
                    .iter()
                    .all(|character| !part.contains(character))
            })
            .collect::<Vec<_>>()
            .join(" "))
    }

    async fn search_items(&self, _library: &Library, path: &str) -> anyhow::Result<Vec<Metadata>> {
        let client = self.get_client()?;
        // let mut url = get_url(&self.url)?.join(&format!("library/sections/{}/all", library.key))?;

        let mut results = vec![];

        let rel_path = path.to_string();

        trace!("searching for item with relative path: {}", rel_path);

        let mut search_term = self.get_search_term(&rel_path)?;

        while !search_term.is_empty() {
            let mut url = get_url(&self.url)?.join("library/search")?;

            url.query_pairs_mut().append_pair("includeCollections", "1");
            url.query_pairs_mut()
                .append_pair("includeExternalMedia", "1");
            url.query_pairs_mut()
                .append_pair("searchTypes", "movies,people,tv");
            url.query_pairs_mut().append_pair("limit", "100");

            trace!("searching for item with term: {}", search_term);

            url.query_pairs_mut()
                // .append_pair("title", search_term.as_str());
                .append_pair("query", search_term.as_str());

            let res = client.get(url).perform().await?;

            let lib: SearchLibraryResponse = res.json().await?;

            let mut metadata = lib
                .media_container
                .search_result
                .into_iter()
                .filter_map(|s| s.metadata)
                .collect::<Vec<_>>();

            // sort episodes then movies to the front, then the rest
            metadata.sort_by(|a, b| {
                if a.t == "episode" && b.t != "episode" {
                    std::cmp::Ordering::Less
                } else if a.t != "episode" && b.t == "episode" {
                    std::cmp::Ordering::Greater
                } else if a.t == "movie" && b.t != "movie" && b.t != "episode" {
                    std::cmp::Ordering::Less
                } else if a.t != "movie" && a.t != "episode" && b.t == "movie" {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            for item in &metadata {
                if item.t == "show" {
                    let episodes = self.get_episodes(&item.key).await?;

                    if let Some(episode_metadata) = episodes.media_container.metadata {
                        for episode in episode_metadata {
                            if let Some(media) = &episode.media {
                                if has_matching_media(media, path) {
                                    results.push(episode.clone());
                                }
                            }
                        }
                    }
                } else if let Some(media) = &item.media {
                    // For movies and other content types
                    if has_matching_media(media, path) {
                        results.push(item.clone());
                    }
                }
            }

            trace!(
                "found {} out of {} items matching search",
                results.len(),
                metadata.len()
            );

            if results.is_empty() {
                let mut search_parts = search_term.split_whitespace().collect::<Vec<_>>();
                search_parts.pop();
                search_term = search_parts.join(" ");
            } else {
                break;
            }
        }

        // if show + episode then remove duplicates
        results.dedup_by_key(|item| item.key.clone());

        Ok(results)
    }

    async fn _get_items(&self, library: &Library, path: &str) -> anyhow::Result<Vec<Metadata>> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("library/sections/{}/all", library.key))?;

        let res = client.get(url).perform().await?;

        let lib: LibraryResponse = res.json().await?;

        let mut parts = vec![];

        // TODO: Reduce the amount of data needed to be searched
        for item in lib.media_container.metadata.unwrap_or_default() {
            match item.t.as_str() {
                "show" => {
                    let episodes = self.get_episodes(&item.key).await?;

                    for episode in episodes.media_container.metadata.unwrap_or_default() {
                        if let Some(media) = &episode.media {
                            if has_matching_media(media, path) {
                                parts.push(episode.clone());
                            }
                        }
                    }
                }
                _ => {
                    if let Some(media) = &item.media {
                        if has_matching_media(media, path) {
                            parts.push(item.clone());
                        }
                    }
                }
            }
        }

        Ok(parts)
    }

    async fn refresh_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("{key}/refresh"))?;

        client.put(url).perform().await.map(|_| ())
    }

    async fn analyze_item(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("{key}/analyze"))?;

        client.put(url).perform().await.map(|_| ())
    }

    async fn scan(&self, ev: &ScanEvent, library: &Library) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let mut url =
            get_url(&self.url)?.join(&format!("library/sections/{}/refresh", library.key))?;

        let ev_path = ev.get_path(&self.rewrite);
        url.query_pairs_mut()
            .append_pair("path", scan_directory(&ev_path));

        client.get(url).perform().await.map(|_| ())
    }

    async fn empty_library_trash(&self, key: &str) -> anyhow::Result<()> {
        let client = self.get_client()?;
        let url = get_url(&self.url)?.join(&format!("library/sections/{key}/emptyTrash"))?;
        client.put(url).perform().await.map(|_| ())
    }
}

impl TargetProcess for Plex {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let libraries = self.libraries().await.context("failed to get libraries")?;
        *self.runtime.libraries.lock().unwrap() = libraries.clone();

        let cleanup_sections = if self.empty_trash {
            evs.iter()
                .flat_map(|ev| {
                    let path = ev.get_path(&self.rewrite);
                    self.get_libraries(&libraries, &path)
                        .into_iter()
                        .map(|library| library.key)
                })
                .collect::<HashSet<_>>()
        } else {
            HashSet::new()
        };
        let cleanup_generations = self.invalidate_cleanups(&cleanup_sections);
        let mut notifications = HashMap::new();
        if !cleanup_sections.is_empty() {
            match self.notification_url() {
                Ok(url) => {
                    for section in &cleanup_sections {
                        match ScanNotifications::connect(url.clone()).await {
                            Ok(watcher) => {
                                notifications.insert(section.clone(), watcher);
                            }
                            Err(error) => error!(
                                "failed to watch Plex section '{section}'; trash cleanup will be skipped: {error:#}"
                            ),
                        }
                    }
                }
                Err(error) => error!(
                    "failed to build Plex notification URL; trash cleanup will be skipped: {error:#}"
                ),
            }
        }

        let mut succeeded: HashMap<String, bool> = HashMap::new();
        let mut cleanups: HashMap<String, LibraryCleanup> = HashMap::new();

        for ev in evs {
            let succeeded_entry = succeeded.entry(ev.id.clone()).or_insert(true);

            let ev_path = ev.get_path(&self.rewrite);
            let matched_libraries = self.get_libraries(&libraries, &ev_path);

            if matched_libraries.is_empty() {
                error!("no matching library for {ev_path}");

                *succeeded_entry = false;

                continue;
            }

            let mut processed_items = HashSet::new();

            for library in matched_libraries {
                trace!("found library '{}' for {ev_path}", library.title);

                let scan_result = self.scan(ev, &library).await;
                if self.empty_trash {
                    let cleanup = cleanups
                        .entry(library.key.clone())
                        .or_insert_with(|| LibraryCleanup { scan_failed: false });
                    cleanup.scan_failed |= scan_result.is_err();
                }

                match scan_result {
                    Ok(()) => {
                        debug!("scanned '{}'", ev_path);

                        if self.analyze || self.refresh {
                            match self.search_items(&library, &ev_path).await {
                                Ok(items) => {
                                    if items.is_empty() {
                                        trace!(
                                            "failed to find items for file: '{}', leaving at scan",
                                            ev_path
                                        );

                                        // scan succeeded, no items to refresh/analyze
                                    } else {
                                        trace!("found items for file '{}'", ev_path);

                                        let mut all_success = true;

                                        for item in items {
                                            let mut item_success = true;

                                            if processed_items.contains(&item.key) {
                                                debug!(
                                                    "already processed item '{}' earlier, skipping",
                                                    item.key
                                                );
                                                continue;
                                            }

                                            if self.refresh {
                                                match self.refresh_item(&item.key).await {
                                                    Ok(()) => {
                                                        debug!("refreshed metadata '{}'", item.key);
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                        "failed to refresh metadata for '{}': {}",
                                                        item.key, e
                                                    );
                                                        item_success = false;
                                                    }
                                                }
                                            }

                                            if self.analyze {
                                                match self.analyze_item(&item.key).await {
                                                    Ok(()) => {
                                                        debug!("analyzed metadata '{}'", item.key);
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                        "failed to analyze metadata for '{}': {}",
                                                        item.key, e
                                                    );
                                                        item_success = false;
                                                    }
                                                }
                                            }

                                            if !item_success {
                                                all_success = false;
                                            }

                                            processed_items.insert(item.key);
                                        }

                                        if !all_success {
                                            *succeeded_entry = false;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("failed to get items for '{}': {:?}", ev_path, e);
                                    *succeeded_entry = false;
                                }
                            };
                        }
                    }
                    Err(e) => {
                        error!("failed to scan file '{}': {}", ev_path, e);
                        *succeeded_entry = false;
                    }
                }
            }
        }

        let mut cleanups = cleanups.into_iter().collect::<Vec<_>>();
        cleanups.sort_by(|(key_a, _), (key_b, _)| key_a.cmp(key_b));

        for (key, cleanup) in cleanups {
            if cleanup.scan_failed {
                error!("skipped trash cleanup for library '{key}': a scan failed");
            } else if let (Some(watcher), Some(generation)) = (
                notifications.remove(&key),
                cleanup_generations.get(&key).copied(),
            ) {
                self.schedule_cleanup(key, generation, watcher);
            } else {
                error!("skipped trash cleanup for library '{key}': notifications are unavailable");
            }
        }

        Ok(succeeded
            .into_iter()
            .filter_map(|(k, v)| if v { Some(k) } else { None })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(value: serde_json::Value) -> Notification {
        serde_json::from_value(serde_json::json!({"NotificationContainer": value})).unwrap()
    }

    fn activity(event: &str, uuid: &str, kind: &str, section: Option<&str>) -> Notification {
        notification(serde_json::json!({
            "ActivityNotification": [{
                "event": event,
                "uuid": uuid,
                "Activity": {
                    "type": kind,
                    "Context": section.map(|section| serde_json::json!({
                        "librarySectionID": section
                    }))
                }
            }]
        }))
    }

    fn test_plex() -> Plex {
        Plex {
            url: String::new(),
            token: String::new(),
            refresh: false,
            analyze: false,
            empty_trash: false,
            rewrite: None,
            filter: PathFilter::default(),
            request: Request::default(),
            runtime: Arc::default(),
        }
    }

    #[test]
    fn scan_notifications_correlate_uuid_with_the_exact_section() {
        let mut state = ScanNotificationState::default();
        // Metadata completion and a completed scan in another section do not count.
        state.apply(activity(
            "ended",
            "metadata",
            "library.update.item.metadata",
            None,
        ));
        state.apply(activity(
            "started",
            "other-scan",
            "library.update.section",
            Some("2"),
        ));
        state.apply(activity(
            "ended",
            "other-scan",
            "library.update.section",
            None,
        ));
        assert!(!state.completion_confirmed("1"));

        // Plex may omit Context on started/ended; the updated event associates the UUID.
        state.apply(activity(
            "started",
            "target-scan",
            "library.update.section",
            None,
        ));
        state.apply(activity(
            "updated",
            "target-scan",
            "library.update.section",
            Some("1"),
        ));
        state.apply(activity(
            "ended",
            "target-scan",
            "library.update.section",
            None,
        ));
        assert!(state.completion_confirmed("1"));
    }

    #[test]
    fn another_active_section_does_not_block_section_cleanup() {
        let mut state = ScanNotificationState::default();
        state.apply(activity(
            "ended",
            "target-scan",
            "library.update.section",
            Some("1"),
        ));
        state.apply(activity(
            "started",
            "other-scan",
            "library.update.section",
            Some("2"),
        ));
        assert!(state.completion_confirmed("1"));
        assert!(!state.completion_confirmed("2"));
    }

    #[test]
    fn unidentified_active_scan_is_ignored_until_its_section_is_known() {
        let mut state = ScanNotificationState::default();
        state.apply(activity(
            "ended",
            "target-scan",
            "library.update.section",
            Some("1"),
        ));
        state.apply(activity(
            "started",
            "unknown-scan",
            "library.update.section",
            None,
        ));
        assert!(state.completion_confirmed("1"));

        state.apply(activity(
            "updated",
            "unknown-scan",
            "library.update.section",
            Some("1"),
        ));
        assert!(!state.completion_confirmed("1"));

        state.apply(activity(
            "ended",
            "unknown-scan",
            "library.update.section",
            None,
        ));
        assert!(state.completion_confirmed("1"));
    }

    #[test]
    fn test_get_search_term() {
        let plex = test_plex();

        // Test with a path that has a file name and season directory
        let path = "/media/TV Shows/Breaking Bad/Season 1/S01E01.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Breaking Bad");

        // Test with a path that has parentheses and brackets
        let path = "/media/Movies/The Matrix (1999) [1080p]/matrix.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "The Matrix");

        // Test with a simple path
        let path = "/media/Movies/Inception/inception.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Inception");

        // Test with a directory path
        let path = "/media/TV Shows/Game of Thrones/Season 2";
        assert_eq!(plex.get_search_term(path).unwrap(), "Game of Thrones");

        // Test with no directory path
        let path = "/media/TV Shows/Game of Thrones";
        assert_eq!(plex.get_search_term(path).unwrap(), "Game of Thrones");

        // Test with multiple levels of season directories
        let path = "/media/TV Shows/Doctor Who/Season 10/Season 10 Part 2/S10E12.mkv";
        assert_eq!(plex.get_search_term(path).unwrap(), "Doctor Who");
    }

    #[test]
    fn test_get_library() {
        let plex = Plex {
            url: String::new(),
            token: String::new(),
            refresh: false,
            analyze: false,
            empty_trash: false,
            rewrite: None,
            filter: PathFilter::default(),
            request: Request::default(),
            runtime: Arc::default(),
        };

        let libraries = [Library {
            title: "Movies".to_string(),
            key: "library_key_movies".to_string(),
            location: vec![Location {
                path: "/media/movies".to_string(),
            }],
        }];

        let path = "/media/movies/Inception.mkv";
        let libraries = plex.get_libraries(&libraries, path);
        assert!(libraries[0].key == "library_key_movies");

        let nested_libraries = [
            Library {
                title: "Movies".to_string(),
                key: "library_key_movies".to_string(),
                location: vec![Location {
                    path: "/media/movies".to_string(),
                }],
            },
            Library {
                title: "Movies".to_string(),
                key: "library_key_movies_4k".to_string(),
                location: vec![Location {
                    path: "/media/movies/4k".to_string(),
                }],
            },
        ];

        let path = "/media/movies/4k/Inception.mkv";

        let libraries = plex.get_libraries(&nested_libraries, path);
        assert!(libraries[0].key == "library_key_movies_4k");
        assert!(libraries[1].key == "library_key_movies");
    }

    #[test]
    fn windows_library_matching_prefers_the_most_specific_location() {
        let libraries = [
            Library {
                title: "Movies".to_string(),
                key: "movies".to_string(),
                location: vec![Location {
                    path: r"\\server\media".to_string(),
                }],
            },
            Library {
                title: "4K Movies".to_string(),
                key: "movies-4k".to_string(),
                location: vec![Location {
                    path: r"\\SERVER\MEDIA\4K".to_string(),
                }],
            },
        ];

        let matches = test_plex().get_libraries(&libraries, r"\\server\media\4k\Film\Film.mkv");

        assert_eq!(
            matches
                .iter()
                .map(|library| library.key.as_str())
                .collect::<Vec<_>>(),
            ["movies-4k", "movies"]
        );
    }

    #[test]
    fn unc_file_produces_a_non_empty_scan_directory_in_original_syntax() {
        assert_eq!(
            scan_directory(r"\\server\media\TV\Show\Season 1\S01E01.mkv"),
            r"\\server\media\TV\Show\Season 1"
        );
    }

    #[test]
    fn windows_search_term_skips_season_components() {
        assert_eq!(
            test_plex()
                .get_search_term(r"D:\TV Shows\Breaking Bad\Season 1\S01E01.mkv")
                .unwrap(),
            "Breaking Bad"
        );
    }

    #[test]
    fn windows_media_matching_uses_runtime_case_and_boundaries() {
        assert!(path_matches(
            r"D:\MEDIA\Movies\Film\Film.mkv",
            r"d:\media\movies\film\film.MKV"
        ));
        assert!(path_matches(
            r"\\server\media\Shows\Show\Episode.mkv",
            r"\\SERVER\MEDIA\SHOWS\SHOW"
        ));
        assert!(!path_matches(
            r"\\server\media-archive\Film.mkv",
            r"\\server\media"
        ));
    }

    #[test]
    fn unix_search_term_without_a_non_season_parent_uses_the_directory_name() {
        assert_eq!(
            test_plex().get_search_term("/Season 1/file.mkv").unwrap(),
            "Season 1"
        );
    }

    #[test]
    fn windows_search_term_without_a_non_season_parent_uses_the_directory_name() {
        assert_eq!(
            test_plex()
                .get_search_term(r"D:\Season 1\S01E01.mkv")
                .unwrap(),
            "Season 1"
        );

        // Same fallback, but called directly on the Season directory rather
        // than a file within it: `parent_or_self` takes its other branch
        // (`is_file()` is false, so the source is used unchanged) and must
        // still resolve to the directory name.
        assert_eq!(
            test_plex().get_search_term(r"D:\Season 1").unwrap(),
            "Season 1"
        );
    }
}
