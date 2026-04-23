//! Environment variable token provider.

use crate::token_provider::TokenProvider;

pub struct EnvProvider;

impl TokenProvider for EnvProvider {
    fn name(&self) -> &'static str {
        "env:NOTION_TOKEN"
    }

    fn load(&self) -> Option<String> {
        std::env::var("NOTION_TOKEN").ok()
    }
}
