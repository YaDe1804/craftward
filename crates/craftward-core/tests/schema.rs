//! The checked-in JSON Schema must match the Rust types.

use std::fs;
use std::path::PathBuf;

#[test]
fn checked_in_schema_matches_the_generated_schema() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/craftward.schema.json");
    let checked_in = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "schema/craftward.schema.json could not be read ({error}). \
             Regenerate it with: cargo run -p craftward-core --features schema --example generate-schema"
        );
    });
    let generated = craftward_core::project_schema_json();
    assert_eq!(
        checked_in, generated,
        "schema/craftward.schema.json does not match the Rust types. \
         Regenerate it with: cargo run -p craftward-core --features schema --example generate-schema"
    );
}
