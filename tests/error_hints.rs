//! Error-hint registry tests — one per new Phase 3 pattern.

use notion_cli::api::error::ApiError;

// Helper: render an ApiError to its Display string.
fn render(e: &ApiError) -> String {
    e.to_string()
}

// 1. relation-unshared
#[test]
fn hint_relation_unshared() {
    let e = ApiError::Validation {
        code: "validation_error".into(),
        message: "Target data source not shared with integration.".into(),
    };
    let s = render(&e);
    assert!(
        s.contains("hint:"),
        "expected hint for relation-unshared: {s}"
    );
    assert!(
        s.contains("Share menu"),
        "expected Share menu hint: {s}"
    );
}

// 2. wiki-parent
#[test]
fn hint_wiki_parent() {
    let e = ApiError::Validation {
        code: "validation_error".into(),
        message: "Cannot create database under wiki.".into(),
    };
    let s = render(&e);
    assert!(s.contains("hint:"), "expected hint for wiki-parent: {s}");
    assert!(
        s.contains("regular page"),
        "expected 'regular page' in hint: {s}"
    );
}

// 3. synced-name-collision
#[test]
fn hint_synced_name_collision() {
    let e = ApiError::Validation {
        code: "validation_error".into(),
        message: "Property name conflicts with synced content.".into(),
    };
    let s = render(&e);
    assert!(
        s.contains("hint:"),
        "expected hint for synced-name-collision: {s}"
    );
    assert!(
        s.contains("synced") || s.contains("Synced"),
        "expected synced reference in hint: {s}"
    );
}

// 4. move-restrictions
#[test]
fn hint_move_restrictions() {
    let e = ApiError::Validation {
        code: "validation_error".into(),
        message: "Cannot move page to target location.".into(),
    };
    let s = render(&e);
    assert!(
        s.contains("hint:"),
        "expected hint for move-restrictions: {s}"
    );
    assert!(
        s.contains("edit access") || s.contains("regular page"),
        "expected move-restriction detail in hint: {s}"
    );
}

// 5. integration-workspace-403 — surfaced via server_error_hint
#[test]
fn hint_integration_workspace_403() {
    use notion_cli::api::error::server_error_hint;
    let hint = server_error_hint(403, "workspace operation not permitted");
    assert!(
        hint.is_some(),
        "expected hint for 403+workspace: got None"
    );
    let h = hint.unwrap();
    assert!(
        h.contains("OAuth") || h.contains("user token"),
        "expected OAuth/user token hint: {h}"
    );
}

// 6. property-filter-id-404 — surfaced via server_error_hint
#[test]
fn hint_property_filter_id_404() {
    use notion_cli::api::error::server_error_hint;
    let hint = server_error_hint(404, "filter_properties value not found");
    assert!(
        hint.is_some(),
        "expected hint for filter_properties 404: got None"
    );
    let h = hint.unwrap();
    assert!(
        h.contains("filter_properties") || h.contains("Property ID"),
        "expected filter_properties hint: {h}"
    );
}
