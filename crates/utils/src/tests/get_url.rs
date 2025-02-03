#[cfg(test)]
mod tests {
    use crate::get_url::get_url;

    #[test]
    fn test_join_no_subpath() -> anyhow::Result<()> {
        let url = "http://example.com".to_string();

        let parsed = get_url(&url)?;

        assert_eq!(parsed.to_string(), "http://example.com/");
        assert_eq!(parsed.join("test")?.to_string(), "http://example.com/test");

        Ok(())
    }

    #[test]
    fn test_join_with_subpath() -> anyhow::Result<()> {
        let url = "http://example.com/test".to_string();

        let parsed = get_url(&url)?;

        assert_eq!(parsed.to_string(), "http://example.com/test/");
        assert_eq!(
            parsed.join("test")?.to_string(),
            "http://example.com/test/test"
        );

        Ok(())
    }
}
