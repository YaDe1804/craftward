//! JSON Schema generated from the project types.
//!
//! This module is compiled only when the `schema` feature is enabled.

use schemars::generate::{SchemaGenerator, SchemaSettings};

use crate::model::Project;

/// Builds the JSON Schema for a schema-version-1 project file.
///
/// The document follows JSON Schema draft 2020-12 and describes the values
/// this crate accepts, including rejection of unknown fields.
///
/// # Panics
///
/// Panics if the generated schema cannot be serialized as JSON. That would be
/// a bug in the schema types.
#[must_use]
pub fn project_schema_json() -> String {
    let settings = SchemaSettings::draft2020_12().for_deserialize();
    let generator = SchemaGenerator::new(settings);
    let schema = generator.into_root_schema_for::<Project>();
    let mut json = serde_json::to_string_pretty(&schema).expect("project schema can be serialized");
    json.push('\n');
    json
}
