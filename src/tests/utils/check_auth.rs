#[cfg(test)]
mod tests {
    use crate::{settings::Settings, utils::check_auth::check_auth};
    use actix_web_httpauth::{extractors::basic::BasicAuth, headers::authorization::Basic};

    #[test]
    fn test_check_default_auth() -> anyhow::Result<()> {
        let auth = BasicAuth::from(Basic::new(
            "admin".to_string(),
            Some("password".to_string()),
        ));
        let settings: Settings = serde_json::from_str("{}")?;

        assert!(check_auth(&auth, &settings));

        Ok(())
    }

    #[test]
    fn test_check_auth_default_incorrect() -> anyhow::Result<()> {
        let auth = BasicAuth::from(Basic::new("username".to_string(), Some("".to_string())));
        let settings: Settings = serde_json::from_str("{}")?;

        assert!(!check_auth(&auth, &settings));

        Ok(())
    }

    #[test]
    fn test_check_auth_username() -> anyhow::Result<()> {
        let auth = BasicAuth::from(Basic::new(
            "username".to_string(),
            Some("password".to_string()),
        ));
        let settings: Settings = serde_json::from_str("{\"auth\":{\"username\":\"username\"}}")?;

        assert!(check_auth(&auth, &settings));

        Ok(())
    }

    #[test]
    fn test_check_auth_password() -> anyhow::Result<()> {
        let auth = BasicAuth::from(Basic::new("admin".to_string(), Some("pass".to_string())));
        let settings: Settings = serde_json::from_str("{\"auth\":{\"password\":\"pass\"}}")?;

        assert!(check_auth(&auth, &settings));

        Ok(())
    }
}
