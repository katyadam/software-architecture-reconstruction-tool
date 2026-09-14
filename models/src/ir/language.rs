#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Java,
    Python,
    Go,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::Java, Self::Python, Self::Go];
}
