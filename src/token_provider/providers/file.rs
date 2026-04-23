//! File-based token provider.
//!
//! Reads the token from the path given by `$NOTION_TOKEN_FILE`, or
//! falls back to `~/.config/notion-cli/token` if the env var is
//! absent.

use crate::token_provider::TokenProvider;

pub struct FileProvider;

impl TokenProvider for FileProvider {
    fn name(&self) -> &'static str {
        "file"
    }

    fn load(&self) -> Option<String> {
        let path = if let Ok(p) = std::env::var("NOTION_TOKEN_FILE") {
            std::path::PathBuf::from(p)
        } else {
            let home = std::env::var("HOME").ok()?;
            std::path::PathBuf::from(home)
                .join(".config")
                .join("notion-cli")
                .join("token")
        };

        std::fs::read_to_string(&path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}
