use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AiProvider {
    Claude,
    Codex,
    Gemini,
}

impl AiProvider {
    pub fn command_name(&self) -> &'static str {
        match self {
            AiProvider::Claude => "claude",
            AiProvider::Codex => "codex",
            AiProvider::Gemini => "gemini",
        }
    }
}
