use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Java,
    Python,
    Unknown,
}

impl Language {
    /// Decide a file's language from its extension.
    ///
    /// The single home for this mapping -- adding a language means adding one
    /// arm here and one arm in the synthesizer's test-path rules.
    pub fn from_path(file_path: &str) -> Self {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str());

        match ext {
            Some("java") => Language::Java,
            Some("py") => Language::Python,
            _ => Language::Unknown,
        }
    }
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Java => write!(f, "Java"),
            Language::Python => write!(f, "Python"),
            Language::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    FastAPI,
    Spring,
    Unknown,
}

impl Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::FastAPI => write!(f, "FastAPI"),
            Framework::Spring => write!(f, "Spring"),
            Framework::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn from_path_maps_known_extensions() {
        assert_eq!(Language::from_path("a/b/Foo.java"), Language::Java);
        assert_eq!(Language::from_path("a/b/foo.py"), Language::Python);
    }

    #[test]
    fn from_path_is_unknown_for_everything_else() {
        assert_eq!(Language::from_path("a/b/main.go"), Language::Unknown);
        assert_eq!(Language::from_path("a/b/README"), Language::Unknown);
        assert_eq!(Language::from_path(""), Language::Unknown);
    }
}
