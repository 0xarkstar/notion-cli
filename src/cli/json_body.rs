//! Universal `--json <body>` parser for Justin Poehnelt
//! agent-first CLI principle #1 (zero-translation agent path).

use std::io::Read;
use std::path::Path;

use crate::cli::CliError;

/// Parse a `--json <body>` argument.
///
/// Modes:
/// - `"-"` → read from stdin until EOF
/// - `"@path"` → read file contents
/// - otherwise → treat as literal JSON string
///
/// Returns the parsed `serde_json::Value` after structural validation
/// (well-formed JSON only).
pub fn parse_json_body(raw: &str) -> Result<serde_json::Value, CliError> {
    let text = if raw == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::Validation(format!("--json stdin: {e}")))?;
        buf
    } else if let Some(path_str) = raw.strip_prefix('@') {
        let path = Path::new(path_str);
        std::fs::read_to_string(path).map_err(|e| {
            CliError::Validation(format!("--json @{}: {e}", path.display()))
        })?
    } else {
        raw.to_string()
    };
    serde_json::from_str(&text).map_err(|e| {
        CliError::Validation(format!("--json body: not valid JSON: {e}"))
    })
}

/// Enforce E8 (NOT silent — reject at parse time with exit 2).
///
/// If the caller supplied `--json`, all `bespoke_flags_present` must
/// be false. Returns Err with exit-2 `CliError::Usage` otherwise.
pub fn reject_json_with_bespoke(
    has_json: bool,
    bespoke_flags_present: &[(&str, bool)],
) -> Result<(), CliError> {
    if !has_json {
        return Ok(());
    }
    let names: Vec<&str> = bespoke_flags_present
        .iter()
        .filter(|(_, present)| *present)
        .map(|(name, _)| *name)
        .collect();
    if names.is_empty() {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "--json is mutually exclusive with {}; remove one",
            names.join(", ")
        )))
    }
}
