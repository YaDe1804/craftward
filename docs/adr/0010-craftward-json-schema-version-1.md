# ADR 0010: craftward.json schema version 1

- Status: Accepted
- Date: 2026-11-15

## Decision

A project file is one JSON object, schema version 1, defined by the types in
`craftward-core`. Field names are English `snake_case`. Unknown fields are
errors, on the root object and on every nested object. Validation collects
every error and names each one by a path such as
`plan.phases[1].tasks[0].due`.

`schema_version` must be the integer `1`. When it is an integer greater than
1, loading stops and reports that the file was written by a newer Craftward
version. The rest of that file is not interpreted. Values that are not
integers are ordinary validation errors.

Cross-field rules that JSON Schema does not express (unique ids, calendar
dates, URL prefixes, relative lockfile paths) are enforced by the Rust
validator. `schema/craftward.schema.json` is generated from the same types and
covers structure, enumerations, required fields, and `additionalProperties:
false`.

## Rationale

One versioned file per project is the source of truth for user-maintained
data. Rejecting unknown fields keeps a newer or mistyped file from being
stored as if this version had understood it. Stopping on a higher
`schema_version`, instead of validating the fields this version happens to
recognize, avoids treating a future file as a partial version-1 document.
