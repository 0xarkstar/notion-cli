//! Eyeball schema output for `PropertyValue` and `Property`.
//!
//! Run: `cargo run --example print_schema`

use notion_cli::types::{Property, PropertyValue};
use schemars::schema_for;

fn main() {
    println!("=== Property (untagged wrapper) ===");
    let wrapper = schema_for!(Property);
    println!("{}", serde_json::to_string_pretty(&wrapper).unwrap());

    println!();
    println!("=== PropertyValue (tagged enum, 22 variants) ===");
    let inner = schema_for!(PropertyValue);
    println!("{}", serde_json::to_string_pretty(&inner).unwrap());
}
