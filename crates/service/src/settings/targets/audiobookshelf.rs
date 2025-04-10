use super::RequestBuilderPerform;
use crate::settings::rewrite::Rewrite;
use crate::settings::{auth::Auth, targets::TargetProcess};
use autopulse_database::models::ScanEvent;
use autopulse_utils::get_url;
use reqwest::header;
use serde::Deserialize;
use tracing::{debug, error};

#[derive(Clone, Deserialize)]
pub struct Audiobookshelf {
    /// URL to the audiobookshelf instance
    pub url: String,
    /// Authentication credentials
    pub auth: Auth,
    /// Trigger to hit (must be type: manual) (default: manual)
    pub trigger: Option<String>,
    /// Rewrite path for the file
    pub rewrite: Option<Rewrite>,
}

#[derive(Clone, Deserialize)]
struct AudiobookshelfUser {
    token: String,
}

#[doc(hidden)]
#[derive(Deserialize)]
struct AudiobookshelfLoginResponse {
    user: AudiobookshelfUser,
}

#[derive(Debug, Deserialize)]
pub struct LibraryFolder {
    #[serde(rename = "fullPath")]
    pub full_path: String,
    #[serde(rename = "libraryId")]
    pub library_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub folders: Vec<LibraryFolder>,
}

#[derive(Debug, Deserialize)]
pub struct LibrariesResponse {
    pub libraries: Vec<Library>,
}

impl Audiobookshelf {
    async fn get_client(&self, token: Option<String>) -> anyhow::Result<reqwest::Client> {
        let mut headers = header::HeaderMap::new();

        if self.auth.enabled {
            if let Some(token) = token {
                headers.insert("Authorization", format!("Bearer {token}").parse()?);
            }
        }

        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(Into::into)
    }

    async fn login(&self) -> anyhow::Result<String> {
        let client = self.get_client(None).await?;
        let url = get_url(&self.url)?.join("login")?;

        let res = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&self.auth)
            .perform()
            .await?;

        let body: AudiobookshelfLoginResponse = res.json().await?;

        Ok(body.user.token)
    }

    async fn scan(&self, token: String, ev: &ScanEvent, library_id: String) -> anyhow::Result<()> {
        let client = self.get_client(Some(token)).await?;
        let url = get_url(&self.url)?.join("api/watcher/update")?;

        client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "libraryId": library_id,
                "path": ev.get_path(&self.rewrite),
                // audiobookshelf will scan for the changes so del/rename *should* be handled
                // https://github.com/mikiher/audiobookshelf/blob/master/server/Watcher.js#L268
                "type": "add"
            }))
            .perform()
            .await
            .map(|_| ())
    }

    async fn get_libraries(&self, token: String) -> anyhow::Result<Vec<Library>> {
        let client = self.get_client(Some(token)).await?;

        let url = get_url(&self.url)?.join("api/libraries")?;

        let res = client.get(url).perform().await?;

        let body: LibrariesResponse = res.json().await?;

        Ok(body.libraries)
    }

    async fn choose_library(
        &self,
        ev: &ScanEvent,
        libraries: &[Library],
    ) -> anyhow::Result<Option<String>> {
        for library in libraries {
            for folder in library.folders.iter() {
                if ev.get_path(&self.rewrite).starts_with(&folder.full_path) {
                    debug!("found library: {}", folder.library_id);
                    return Ok(Some(folder.library_id.clone()));
                }
            }
        }

        Ok(None)
    }
}

impl TargetProcess for Audiobookshelf {
    async fn process(&self, evs: &[&ScanEvent]) -> anyhow::Result<Vec<String>> {
        let mut succeded = Vec::new();
        let token = self.login().await?;

        let libraries = self.get_libraries(token.clone()).await?;

        if libraries.is_empty() {
            error!("no libraries found");
            return Ok(succeded);
        }

        for ev in evs {
            match self.choose_library(ev, &libraries).await {
                Ok(Some(library_id)) => {
                    if let Err(e) = self.scan(token.clone(), ev, library_id).await {
                        error!("failed to scan audiobookshelf: {}", e);
                    } else {
                        succeded.push(ev.get_path(&self.rewrite));
                    }
                }
                Ok(None) => {
                    error!("no library found for {}", ev.get_path(&self.rewrite));
                }
                Err(e) => {
                    error!("failed to choose library: {}", e);
                }
            }
        }

        Ok(succeded)
    }
}
