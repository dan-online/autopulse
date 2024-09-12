use serde::Deserialize;

#[derive(Deserialize)]
pub struct ManualQueryParams {
    /// Path to the file
    pub path: String,
    /// Optional hash of the file
    pub hash: Option<String>,
}
