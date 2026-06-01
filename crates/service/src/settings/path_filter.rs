use autopulse_utils::regex::Regex;
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PathFilter {
    /// Path regex patterns to include. Empty includes all paths unless excluded.
    #[serde(default)]
    pub include: IncludePaths,
    /// Path regex patterns to exclude. Excludes win over includes.
    #[serde(default)]
    pub exclude: ExcludePaths,
}

impl PathFilter {
    pub fn allows(&self, path: &str) -> bool {
        (self.include.is_empty() || self.include.matches(path)) && !self.exclude.matches(path)
    }
}

#[derive(Clone, Default)]
pub struct IncludePaths {
    patterns: Vec<Regex>,
    sources: Vec<String>,
}

impl IncludePaths {
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn matches(&self, path: &str) -> bool {
        self.patterns.iter().any(|r| r.is_match(path))
    }
}

impl<'de> Deserialize<'de> for IncludePaths {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let sources: Vec<String> = Vec::<String>::deserialize(d)?;
        let mut patterns = Vec::with_capacity(sources.len());
        for s in &sources {
            let r = Regex::new(s).map_err(|e| {
                D::Error::custom(format!("invalid filter.include regex `{s}`: {e}"))
            })?;
            patterns.push(r);
        }
        Ok(Self { patterns, sources })
    }
}

impl Serialize for IncludePaths {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.sources.serialize(s)
    }
}

#[derive(Clone, Default)]
pub struct ExcludePaths {
    patterns: Vec<Regex>,
    sources: Vec<String>,
}

impl ExcludePaths {
    pub fn matches(&self, path: &str) -> bool {
        self.patterns.iter().any(|r| r.is_match(path))
    }
}

impl<'de> Deserialize<'de> for ExcludePaths {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let sources: Vec<String> = Vec::<String>::deserialize(d)?;
        let mut patterns = Vec::with_capacity(sources.len());
        for s in &sources {
            let r = Regex::new(s).map_err(|e| {
                D::Error::custom(format!("invalid filter.exclude regex `{s}`: {e}"))
            })?;
            patterns.push(r);
        }
        Ok(Self { patterns, sources })
    }
}

impl Serialize for ExcludePaths {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.sources.serialize(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_match_any_pattern() {
        let filter: PathFilter = serde_json::from_value(serde_json::json!({
            "include": ["^/books/", "^/podcasts/"]
        }))
        .unwrap();

        assert!(filter.allows("/books/Novel.m4b"));
        assert!(filter.allows("/podcasts/Episode.mp3"));
        assert!(!filter.allows("/movies/Movie.mkv"));
    }

    #[test]
    fn excludes_match_any_pattern() {
        let filter: PathFilter = serde_json::from_value(serde_json::json!({
            "exclude": ["/samples/", "\\.tmp$"]
        }))
        .unwrap();

        assert!(!filter.allows("/books/samples/Sample.m4b"));
        assert!(!filter.allows("/books/file.tmp"));
        assert!(filter.allows("/books/Novel.m4b"));
    }

    #[test]
    fn exclude_wins_over_include() {
        let filter: PathFilter = serde_json::from_value(serde_json::json!({
            "include": ["^/books/"],
            "exclude": ["^/books/samples/"]
        }))
        .unwrap();

        assert!(filter.allows("/books/Novel.m4b"));
        assert!(!filter.allows("/books/samples/Sample.m4b"));
    }

    #[test]
    fn invalid_include_regex_rejected_at_config_load() {
        let res: Result<PathFilter, _> = serde_json::from_value(serde_json::json!({
            "include": ["[unclosed"]
        }));

        assert!(res.is_err());
        let msg = format!("{}", res.err().unwrap());
        assert!(msg.contains("invalid filter.include regex"));
    }

    #[test]
    fn invalid_exclude_regex_rejected_at_config_load() {
        let res: Result<PathFilter, _> = serde_json::from_value(serde_json::json!({
            "exclude": ["[unclosed"]
        }));

        assert!(res.is_err());
        let msg = format!("{}", res.err().unwrap());
        assert!(msg.contains("invalid filter.exclude regex"));
    }
}
