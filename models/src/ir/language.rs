#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Java,
    Python,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    FastAPI,
    Spring,
    Unknown,
}
