//! Exec-based token provider.
//!
//! Runs the command in `$NOTION_TOKEN_CMD` and uses stdout as the
//! token. The command is split on whitespace for argv (no shell
//! expansion). Returns `None` if the env var is absent or the command
//! fails.

use crate::token_provider::TokenProvider;

pub struct ExecProvider;

impl TokenProvider for ExecProvider {
    fn name(&self) -> &'static str {
        "exec:NOTION_TOKEN_CMD"
    }

    fn load(&self) -> Option<String> {
        let cmd_str = std::env::var("NOTION_TOKEN_CMD").ok()?;
        let mut parts = cmd_str.split_whitespace();
        let program = parts.next()?;
        let args: Vec<&str> = parts.collect();

        let output = std::process::Command::new(program)
            .args(&args)
            .output()
            .ok()?;

        if output.status.success() {
            let token = String::from_utf8(output.stdout).ok()?;
            let trimmed = token.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        } else {
            None
        }
    }
}
