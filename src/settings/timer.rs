use serde::Deserialize;

#[derive(Deserialize, Clone, Default)]
pub struct Timer {
    /// Time to wait before processing
    pub wait: Option<u64>,
}
