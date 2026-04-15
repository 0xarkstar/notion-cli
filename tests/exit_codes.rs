//! Exit code mapping unit tests.
//!
//! CLI exit codes are part of the public contract (stable for
//! downstream tooling). Every variant of [`CliError`] must map to a
//! specific documented code — see the table in `src/cli/mod.rs`.

use notion_cli::api::ApiError;
use notion_cli::cli::CliError;

#[test]
fn unauthorized_maps_to_10() {
    let e = CliError::Api(ApiError::Unauthorized);
    assert_eq!(e.exit_code(), 10);
}

#[test]
fn validation_from_api_maps_to_2() {
    let e = CliError::Api(ApiError::Validation {
        code: "validation_error".into(),
        message: "msg".into(),
    });
    assert_eq!(e.exit_code(), 2);
}

#[test]
fn rate_limited_maps_to_4() {
    let e = CliError::Api(ApiError::RateLimited { retry_after: Some(1) });
    assert_eq!(e.exit_code(), 4);
}

#[test]
fn not_found_maps_to_3() {
    assert_eq!(CliError::Api(ApiError::NotFound).exit_code(), 3);
}

#[test]
fn body_too_large_maps_to_3() {
    assert_eq!(
        CliError::Api(ApiError::BodyTooLarge { limit_bytes: 1024 }).exit_code(),
        3,
    );
}

#[test]
fn server_error_maps_to_3() {
    let e = CliError::Api(ApiError::ServerError {
        status: 503,
        message: "unavailable".into(),
    });
    assert_eq!(e.exit_code(), 3);
}

#[test]
fn network_error_maps_to_3() {
    let e = CliError::Api(ApiError::Network {
        kind: "timeout",
        message: "slow".into(),
    });
    assert_eq!(e.exit_code(), 3);
}

#[test]
fn api_json_error_maps_to_65() {
    let bad_json: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let e = CliError::Api(ApiError::Json(bad_json));
    assert_eq!(e.exit_code(), 65);
}

#[test]
fn cli_validation_maps_to_2() {
    let e = CliError::Validation("bad id".into());
    assert_eq!(e.exit_code(), 2);
}

#[test]
fn config_maps_to_10() {
    let e = CliError::Config("no token".into());
    assert_eq!(e.exit_code(), 10);
}

#[test]
fn usage_maps_to_64() {
    let e = CliError::Usage("bad args".into());
    assert_eq!(e.exit_code(), 64);
}

#[test]
fn io_maps_to_74() {
    let e = CliError::Io(std::io::Error::other("disk"));
    assert_eq!(e.exit_code(), 74);
}

#[test]
fn json_maps_to_65() {
    let bad: serde_json::Error =
        serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let e = CliError::Json(bad);
    assert_eq!(e.exit_code(), 65);
}

#[test]
fn display_never_exposes_raw_inner_token() {
    // CliError wraps ApiError which must not leak auth info.
    let e = CliError::Api(ApiError::Unauthorized);
    let s = format!("{e}");
    assert!(!s.is_empty());
    assert!(!s.contains("Bearer"));
    assert!(!s.contains("ntn_"));
}
