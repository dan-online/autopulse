use crate::settings::rewrite::Rewrite;
use crate::settings::timer::Timer;
use crate::settings::triggers::TriggerRequest;
use autopulse_utils::join_path::join_path;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Radarr {
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
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct MovieFile {
    pub relative_path: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[doc(hidden)]
pub struct Movie {
    pub folder_path: String,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "eventType")]
#[doc(hidden)]
pub enum RadarrRequest {
    #[serde(rename = "Download")]
    #[serde(rename_all = "camelCase")]
    Download { movie_file: MovieFile, movie: Movie },
    #[serde(rename = "MovieDelete")]
    #[serde(rename_all = "camelCase")]
    MovieDelete { movie: Movie },
    #[serde(rename = "MovieFileDelete")]
    #[serde(rename_all = "camelCase")]
    MovieFileDelete { movie_file: MovieFile, movie: Movie },
    #[serde(rename = "Rename")]
    #[serde(rename_all = "camelCase")]
    Rename { movie: Movie },
    #[serde(rename = "Test")]
    Test,
}

impl TriggerRequest for RadarrRequest {
    fn from_json(json: serde_json::Value) -> anyhow::Result<Self> {
        serde_json::from_value(json).map_err(|e| anyhow::anyhow!(e))
    }
    fn paths(&self) -> Vec<(String, bool)> {
        match self {
            Self::MovieFileDelete { movie, movie_file } => {
                vec![(
                    join_path(&movie.folder_path, &movie_file.relative_path),
                    false,
                )]
            }
            Self::Rename { movie } => {
                vec![(movie.folder_path.clone(), true)]
            }
            Self::MovieDelete { movie } => {
                vec![(movie.folder_path.clone(), false)]
            }
            Self::Download { movie, movie_file } => {
                vec![(
                    join_path(&movie.folder_path, &movie_file.relative_path),
                    true,
                )]
            }
            Self::Test => vec![],
        }
    }
}
