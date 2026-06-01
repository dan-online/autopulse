use base64::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Auth {
    /// Whether authentication is enabled (default: true)
    #[serde(skip_serializing)]
    pub enabled: bool,
    /// Username for basic auth (default: admin)
    pub username: String,
    /// Password for basic auth (default: password)
    pub password: String,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            enabled: true,
            username: "admin".to_string(),
            password: "password".to_string(),
        }
    }
}

impl Auth {
    pub fn to_auth_encoded(&self) -> String {
        format!(
            "Basic {}",
            BASE64_STANDARD.encode(format!("{}:{}", self.username, self.password))
        )
    }

    pub fn is_default_credentials(&self) -> bool {
        let defaults = Self::default();
        self.enabled && self.username == defaults.username && self.password == defaults.password
    }
}
