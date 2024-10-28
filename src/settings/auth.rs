use base64::prelude::*;
use serde::Deserialize;

#[doc(hidden)]
fn default_username() -> String {
    "admin".to_string()
}

#[doc(hidden)]
fn default_password() -> String {
    "password".to_string()
}

/// Authentication settings
///
/// Example:
///
/// ```yml
/// auth:
///   username: terry
///   password: yogurt
/// ```
#[derive(Deserialize, Clone, Debug)]
pub struct Auth {
    /// Username for basic auth (default: admin)
    #[serde(default = "default_username")]
    pub username: String,
    /// Password for basic auth (default: password)
    #[serde(default = "default_password")]
    pub password: String,
}

impl Default for Auth {
    fn default() -> Self {
        Auth {
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
