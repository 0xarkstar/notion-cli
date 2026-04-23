//! Token resolution chain — env → file → keychain → exec.
//!
//! First Ok wins. Emits a stderr warning when multiple backends
//! resolve to a value (shadowing). Chain order overridable via
//! `NOTION_CLI_TOKEN_CHAIN` env (e.g. `keychain,env`).
//!
//! # Naming note
//!
//! The CLI `auth` namespace is reserved for v0.6 OAuth work. This
//! internal module is `token_provider` to avoid the collision.

pub mod providers;

use crate::config::NotionToken;

pub use providers::env::EnvProvider;
pub use providers::exec::ExecProvider;
pub use providers::file::FileProvider;
pub use providers::keychain::KeychainProvider;

/// Trait implemented by every token backend.
pub trait TokenProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn load(&self) -> Option<String>;
}

/// Ordered chain of providers — first non-None wins.
pub struct TokenChain {
    providers: Vec<Box<dyn TokenProvider>>,
}

impl TokenChain {
    /// Default chain order: env → file → keychain → exec.
    #[must_use]
    pub fn default_chain() -> Self {
        Self {
            providers: vec![
                Box::new(EnvProvider),
                Box::new(FileProvider),
                Box::new(KeychainProvider),
                Box::new(ExecProvider),
            ],
        }
    }

    /// Resolve a token, emitting a stderr warning if multiple
    /// providers have a value.
    pub fn resolve(&self) -> Option<NotionToken> {
        let mut found: Option<(&'static str, String)> = None;
        let mut shadows: Vec<&'static str> = Vec::new();

        for p in &self.providers {
            if let Some(t) = p.load() {
                if found.is_some() {
                    shadows.push(p.name());
                } else {
                    found = Some((p.name(), t));
                }
            }
        }

        if let Some((winner, token)) = found {
            if !shadows.is_empty() {
                eprintln!(
                    "notion-cli: warning: {} token shadows {} entries; using {}",
                    winner,
                    shadows.join(", "),
                    winner
                );
            }
            Some(NotionToken::new(token))
        } else {
            None
        }
    }
}
