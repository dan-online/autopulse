use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Rewrite {
    /// Path to rewrite from
    pub from: String,
    /// Path to rewrite to
    pub to: String,
}

impl Rewrite {
    pub fn rewrite_path(&self, path: String) -> String {
        let from_regex = Regex::new(self.from.as_str()).expect("Invalid regex in 'from' field");
        let result = from_regex.replace(&path, self.to.as_str()).to_string();

        result
    }
}
