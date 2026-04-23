//! macOS Keychain token provider.
//!
//! On macOS, reads the "notion-cli" Keychain item via the
//! `security` command-line tool (avoids adding a native dep for v0.4;
//! `security-framework` integration is deferred to v0.5).
//!
//! On non-macOS platforms this provider is a no-op (always returns
//! `None`).

use crate::token_provider::TokenProvider;

pub struct KeychainProvider;

impl TokenProvider for KeychainProvider {
    fn name(&self) -> &'static str {
        "keychain"
    }

    fn load(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            // Use the `security` CLI — avoids adding security-framework dep
            // for v0.4. Native SDK integration deferred to v0.5.
            let output = std::process::Command::new("security")
                .args([
                    "find-generic-password",
                    "-s",
                    "notion-cli",
                    "-w", // print password only
                ])
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
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
}
