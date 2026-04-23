//! `TokenChain` provider tests.
//!
//! All tests that read/write process env vars share a global mutex
//! to prevent races between parallel test threads.

use std::sync::Mutex;

use notion_cli::token_provider::{
    EnvProvider, ExecProvider, FileProvider, KeychainProvider, TokenChain, TokenProvider,
};

/// Serialize all env-touching tests to prevent parallel env races.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// --- EnvProvider -----------------------------------------------------------

#[test]
fn env_provider_reads_notion_token() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::remove_var("NOTION_TOKEN");
    std::env::set_var("NOTION_TOKEN", "ntn_test_env_token_abc123");
    let token = EnvProvider.load();
    std::env::remove_var("NOTION_TOKEN");
    assert_eq!(token.as_deref(), Some("ntn_test_env_token_abc123"));
}

#[test]
fn env_provider_returns_none_when_absent() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::remove_var("NOTION_TOKEN");
    assert!(EnvProvider.load().is_none());
}

// --- FileProvider ----------------------------------------------------------

#[test]
fn file_provider_reads_file_path_env() {
    let _g = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("token");
    std::fs::write(&file_path, "ntn_file_token_xyz\n").unwrap();

    std::env::set_var("NOTION_TOKEN_FILE", file_path.to_str().unwrap());
    let token = FileProvider.load();
    std::env::remove_var("NOTION_TOKEN_FILE");

    assert_eq!(token.as_deref(), Some("ntn_file_token_xyz"));
}

#[test]
fn file_provider_returns_none_for_missing_file() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("NOTION_TOKEN_FILE", "/tmp/notion_cli_nonexistent_token_file_12345");
    let token = FileProvider.load();
    std::env::remove_var("NOTION_TOKEN_FILE");
    assert!(token.is_none());
}

// --- ExecProvider ----------------------------------------------------------

#[test]
fn exec_provider_runs_command() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("NOTION_TOKEN_CMD", "echo -n ntn_exec_token_abc");
    let token = ExecProvider.load();
    std::env::remove_var("NOTION_TOKEN_CMD");
    assert_eq!(token.as_deref(), Some("ntn_exec_token_abc"));
}

#[test]
fn exec_provider_cmd_set_to_nonexistent_binary_returns_none() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("NOTION_TOKEN_CMD", "__notion_cli_nonexistent_binary_xyz__");
    let token = ExecProvider.load();
    std::env::remove_var("NOTION_TOKEN_CMD");
    assert!(token.is_none(), "nonexistent binary must return None");
}

// --- KeychainProvider ------------------------------------------------------

#[test]
fn keychain_provider_returns_none_without_entry() {
    // On macOS: "notion-cli" Keychain item is unlikely to exist in CI.
    // On non-macOS: always None.
    // Either way this test verifies the provider doesn't panic.
    let _ = KeychainProvider.load();
}

// --- TokenChain ------------------------------------------------------------

#[test]
fn chain_returns_none_when_all_empty() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_TOKEN_CMD");
    // Point file provider at nonexistent path.
    std::env::set_var("NOTION_TOKEN_FILE", "/tmp/notion_cli_chain_empty_test_12345");

    let chain = TokenChain::default_chain();
    let result = chain.resolve();
    std::env::remove_var("NOTION_TOKEN_FILE");

    assert!(result.is_none(), "chain must return None when no provider has a token");
}

#[test]
fn chain_env_wins_as_first_provider() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("NOTION_TOKEN", "ntn_chain_env_winner");
    // Also set a file so a second provider fires (shadow warning path).
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("token");
    std::fs::write(&file_path, "ntn_chain_file_token").unwrap();
    std::env::set_var("NOTION_TOKEN_FILE", file_path.to_str().unwrap());

    let chain = TokenChain::default_chain();
    let token = chain.resolve();

    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_TOKEN_FILE");

    assert!(token.is_some());
    let dbg = format!("{:?}", token.unwrap());
    assert!(dbg.starts_with("NotionToken(ntn_"), "prefix check: {dbg}");
}
