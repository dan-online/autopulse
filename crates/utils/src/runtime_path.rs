use std::fmt;
use typed_path::Utf8TypedPath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimePathFlavor {
    Unix,
    Windows,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimePath<'a> {
    source: &'a str,
    inner: Utf8TypedPath<'a>,
}

#[derive(Clone, Copy)]
enum CaseSensitivity {
    Sensitive,
    AsciiInsensitive,
}

impl<'a> RuntimePath<'a> {
    pub fn new(path: &'a str) -> Self {
        Self {
            source: path,
            inner: Utf8TypedPath::derive(path),
        }
    }

    pub fn flavor(&self) -> RuntimePathFlavor {
        if self.inner.is_unix() {
            RuntimePathFlavor::Unix
        } else {
            RuntimePathFlavor::Windows
        }
    }

    pub fn as_str(&self) -> &'a str {
        self.source
    }

    pub fn starts_with(&self, base: RuntimePath<'_>) -> bool {
        self.starts_with_mode(base, self.flavor().default_sensitivity())
    }

    pub fn starts_with_case_sensitive(&self, base: RuntimePath<'_>) -> bool {
        self.starts_with_mode(base, CaseSensitivity::Sensitive)
    }

    pub fn starts_with_ascii_case_insensitive(&self, base: RuntimePath<'_>) -> bool {
        self.starts_with_mode(base, CaseSensitivity::AsciiInsensitive)
    }

    /// Named rather than implemented via `PartialEq` because path equality
    /// here is a policy decision, not a plain value comparison: it silently
    /// applies a flavor-dependent case-folding rule (ASCII-insensitive on
    /// Windows, sensitive on Unix). Callers should opt into that policy
    /// explicitly by name rather than pick it up implicitly through `==`.
    pub fn equals(&self, other: RuntimePath<'_>) -> bool {
        if self.flavor() != other.flavor() {
            return false;
        }

        let sensitivity = self.flavor().default_sensitivity();

        let mut self_components = self.inner.components();
        let mut other_components = other.inner.components();

        loop {
            match (self_components.next(), other_components.next()) {
                (Some(a), Some(b)) => {
                    if !sensitivity.components_match(a.as_str(), b.as_str()) {
                        return false;
                    }
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }

    pub fn component_count(&self) -> usize {
        self.inner.components().count()
    }

    pub fn normal_components(&self) -> impl DoubleEndedIterator<Item = &str> + '_ {
        self.inner
            .components()
            .filter(|component| component.is_normal())
            .map(|component| component.as_str())
    }

    pub fn file_name(&self) -> Option<&str> {
        self.inner.file_name()
    }

    /// Classifies by the presence of a file extension. This is a syntactic
    /// check on the path string, not a filesystem stat: it never touches
    /// disk and cannot detect an extant directory named e.g. `Season.1` or
    /// a genuinely extension-less file.
    pub fn is_file(&self) -> bool {
        self.inner.extension().is_some()
    }

    pub fn is_directory(&self) -> bool {
        !self.is_file()
    }

    /// Reconstructs the parent from a prefix of the original `source`
    /// string, rather than returning `Utf8TypedPath::parent()` directly, so
    /// the result keeps borrowing the caller's `'a` input and preserves the
    /// source's exact original rendering instead of a re-serialized one.
    pub fn parent_or_self(&self) -> RuntimePath<'a> {
        let source = if self.is_file() {
            let length = self
                .inner
                .parent()
                .map_or(self.source.len(), |parent| parent.as_str().len());
            &self.source[..length]
        } else {
            self.source
        };
        RuntimePath::new(source)
    }

    fn starts_with_mode(&self, base: RuntimePath<'_>, sensitivity: CaseSensitivity) -> bool {
        if self.flavor() != base.flavor() {
            return false;
        }

        let mut components = self.inner.components();
        base.inner.components().all(|base_component| {
            components.next().is_some_and(|component| {
                sensitivity.components_match(component.as_str(), base_component.as_str())
            })
        })
    }
}

impl RuntimePathFlavor {
    /// The default case-sensitivity policy for this flavor: Unix paths
    /// compare byte-for-byte; Windows paths fold ASCII case, mirroring
    /// each platform's native filesystem semantics.
    fn default_sensitivity(self) -> CaseSensitivity {
        match self {
            RuntimePathFlavor::Unix => CaseSensitivity::Sensitive,
            RuntimePathFlavor::Windows => CaseSensitivity::AsciiInsensitive,
        }
    }
}

impl CaseSensitivity {
    fn components_match(self, a: &str, b: &str) -> bool {
        match self {
            CaseSensitivity::Sensitive => a == b,
            CaseSensitivity::AsciiInsensitive => a.eq_ignore_ascii_case(b),
        }
    }
}

impl fmt::Display for RuntimePath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
