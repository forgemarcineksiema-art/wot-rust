#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderLimitProfile {
    #[default]
    LowSpec,
    Recommended,
    High,
}

impl RenderLimitProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LowSpec => "low-spec",
            Self::Recommended => "recommended",
            Self::High => "high",
        }
    }
}
