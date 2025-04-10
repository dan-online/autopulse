use base64::prelude::*;
use serde::{Deserialize, Serialize};

#[doc(hidden)]
fn default_username() -> String {
    "admin".to_string()
}

#[doc(hidden)]
fn default_password() -> String {
    "password".to_string()
}

#[doc(hidden)]
const fn default_enabled() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Auth {
    /// Whether authentication is enabled (default: true)
    #[serde(default = "default_enabled")]
    #[serde(skip_serializing)]
    pub enabled: bool,
    /// Username for basic auth (default: admin)
    #[serde(default = "default_username")]
    pub username: String,
    /// Password for basic auth (default: password)
    #[serde(default = "default_password")]
    pub password: String,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            username: default_username(),
            password: default_password(),
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
}
