use regex::Regex;
use serde::Deserialize;

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
#[derive(Clone)]
pub struct Rewrite {
    pub(crate) rewrites: Vec<SingleRewrite>,
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

        Ok(Self { rewrites })
    }
}

#[derive(Deserialize, Clone)]
pub struct SingleRewrite {
    /// Path to rewrite from
    pub from: String,
    /// Path to rewrite to
    pub to: String,
}

impl Rewrite {
    pub fn rewrite_path(&self, path: String) -> String {
        let mut result = path;

        for rewrite in &self.rewrites {
            let from_regex = Regex::new(&rewrite.from).expect("Invalid regex in 'from' field");
            result = from_regex.replace_all(&result, &rewrite.to).to_string();
        }

        result
    }

    #[cfg(test)]
    pub fn single(from: &str, to: &str) -> Self {
        Self {
            rewrites: vec![SingleRewrite {
                from: from.to_string(),
                to: to.to_string(),
            }],
        }
    }

    #[cfg(test)]
    pub fn multiple(rewrites: Vec<(&str, &str)>) -> Self {
        Self {
            rewrites: rewrites
                .into_iter()
                .map(|(from, to)| SingleRewrite {
                    from: from.to_string(),
                    to: to.to_string(),
                })
                .collect(),
        }
    }
}
