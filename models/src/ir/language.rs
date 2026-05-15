use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Java,
    Python,
    Unknown,
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
