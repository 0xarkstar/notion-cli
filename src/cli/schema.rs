//! `notion-cli schema <type>` — print JSON Schema for an internal type.
//!
//! Useful for agents to learn the shape of Notion property values,
//! filter expressions, sort criteria, etc. without parsing DESIGN.md
//! or the crate source.

use clap::{Args, ValueEnum};
use schemars::schema_for;

use crate::cli::{Cli, CliError};

#[derive(Args, Debug)]
pub struct SchemaArgs {
    /// Which internal type to introspect.
    #[arg(value_enum)]
    pub ty: SchemaType,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SchemaType {
    /// `Property` wrapper (tagged `PropertyValue` with `Raw` fallback).
    Property,
    /// `PropertyValue` — all 22 variants, discriminated by `type`.
    PropertyValue,
    /// `FilterExpression` — recursive filter model.
    Filter,
    /// `SortCriterion`.
    Sort,
    /// `Page` object.
    Page,
    /// `Database` container object.
    Database,
    /// `DataSource` object.
    DataSource,
    /// Rich text run.
    RichText,
}

pub fn run(cli: &Cli, args: &SchemaArgs) -> Result<(), CliError> {
    use crate::types::{
        Database, DataSource, FilterExpression, Page, Property, PropertyValue, SortCriterion,
    };
    use crate::types::rich_text::RichText;

    let schema = match args.ty {
        SchemaType::Property => serde_json::to_value(schema_for!(Property))?,
        SchemaType::PropertyValue => serde_json::to_value(schema_for!(PropertyValue))?,
        SchemaType::Filter => serde_json::to_value(schema_for!(FilterExpression))?,
        SchemaType::Sort => serde_json::to_value(schema_for!(SortCriterion))?,
        SchemaType::Page => serde_json::to_value(schema_for!(Page))?,
        SchemaType::Database => serde_json::to_value(schema_for!(Database))?,
        SchemaType::DataSource => serde_json::to_value(schema_for!(DataSource))?,
        SchemaType::RichText => serde_json::to_value(schema_for!(RichText))?,
    };
    // Schema introspection output is crate-internal, not Notion-origin —
    // always raw (no untrusted envelope), but honours --pretty.
    let serialised = if cli.pretty {
        serde_json::to_string_pretty(&schema)?
    } else {
        serde_json::to_string(&schema)?
    };
    println!("{serialised}");
    Ok(())
}
