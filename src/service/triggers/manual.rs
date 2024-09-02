use serde::Deserialize;

#[derive(Deserialize)]
pub struct ManualQueryParams {
    pub path: String,
    pub hash: Option<String>,
}
