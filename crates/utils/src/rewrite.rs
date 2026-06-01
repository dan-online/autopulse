use regex::Regex;
use serde::{Deserialize, Serialize};

/// Rewrites
///
/// This struct allows for a flexible way to define path rewrites using regex.
///
/// It can handle both single and multiple rewrites, where each rewrite consists of a `from` regex pattern and a `to` replacement string.
///
/// ```yml
/// rewrite:
///   from: '^/old/path/(.*)$'
///   to: '/new/path/$1'
/// ```
/// or
/// ```yml
/// rewrite:
///   - from: /testing
///     to: /production
///   - from: '^/old/path/(.*)$'
///     to: '/new/path/$1'
/// ``````
#[derive(Serialize, Clone)]
pub struct Rewrite {
    pub(crate) rewrites: Vec<SingleRewrite>,
    #[serde(skip)]
    compiled: Vec<(Regex, String)>,
}

impl<'de> Deserialize<'de> for Rewrite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Inner {
            Single(SingleRewrite),
            Multiple(Vec<SingleRewrite>),
        }

        let inner = Inner::deserialize(deserializer)?;

        let rewrites = match inner {
            Inner::Single(single) => {
                vec![single]
            }
            Inner::Multiple(multiple) => multiple,
        };

        let compiled = rewrites
            .iter()
            .map(|r| {
                let re = Regex::new(&r.from).map_err(|e| {
                    serde::de::Error::custom(format!(
                        "invalid regex '{}' in rewrite 'from' field: {e}",
                        r.from
                    ))
                })?;
                Ok((re, r.to.clone()))
            })
            .collect::<Result<Vec<_>, D::Error>>()?;

        Ok(Self { rewrites, compiled })
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SingleRewrite {
    /// Path to rewrite from
    pub from: String,
    /// Path to rewrite to
    pub to: String,
}

impl Rewrite {
    pub fn rewrite_path(&self, path: String) -> String {
        let mut result = path;

        for (from_regex, to) in &self.compiled {
            result = from_regex.replace_all(&result, to).to_string();
        }

        result
    }

    #[cfg(test)]
    pub fn single(from: &str, to: &str) -> Self {
        let rewrites = vec![SingleRewrite {
            from: from.to_string(),
            to: to.to_string(),
        }];
        // In test helpers, an invalid pattern should fail loudly rather
        // than silently produce a no-op rewrite that hides setup bugs.
        let compiled = rewrites
            .iter()
            .map(|r| {
                let re = Regex::new(&r.from)
                    .unwrap_or_else(|e| panic!("invalid regex {:?} in test helper: {e}", r.from));
                (re, r.to.clone())
            })
            .collect();
        Self { rewrites, compiled }
    }

    #[cfg(test)]
    pub fn multiple(rewrites: Vec<(&str, &str)>) -> Self {
        let rewrites: Vec<SingleRewrite> = rewrites
            .into_iter()
            .map(|(from, to)| SingleRewrite {
                from: from.to_string(),
                to: to.to_string(),
            })
            .collect();
        let compiled = rewrites
            .iter()
            .map(|r| {
                let re = Regex::new(&r.from)
                    .unwrap_or_else(|e| panic!("invalid regex {:?} in test helper: {e}", r.from));
                (re, r.to.clone())
            })
            .collect();
        Self { rewrites, compiled }
    }
}
