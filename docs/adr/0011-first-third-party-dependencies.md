# ADR 0011: First third-party dependencies

- Status: Accepted
- Date: 2026-11-15

## Decision

`craftward-core` depends on three crates, reviewed against crates.io on
2026-11-15. Features are limited to `std` and `derive`. Default features of
each crate are turned off so optional integrations (ordered maps, `chrono`,
`uuid`, `url`, and similar) are not pulled in.

| Crate | Version | License | Features |
| --- | --- | --- | --- |
| serde | 1.0.229 | MIT OR Apache-2.0 | `derive`, `std` |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | `std` |
| schemars | 1.2.3 | MIT | `derive`, `std` |

`serde` and `serde_json` are normal dependencies. They parse and deserialize
project files, including a line and column for syntax errors. `schemars` is
optional and is enabled only by the `schema` feature of `craftward-core`.
That feature is not a default feature. The command-line tool and the desktop
app do not turn it on, so `schemars` is not part of their dependency graph.
It is used to generate `schema/craftward.schema.json` from the same Rust
types the parser uses:

```console
cargo run -p craftward-core --features schema --example generate-schema
```

The running tools do not need that generator. The schema file is checked in,
and validation uses `serde` and `serde_json`. Dates stay hand-written strings.
Errors implement `std::error::Error` directly. The command-line parser stays
in the standard library.

A hand-written JSON parser was considered and rejected. JSON strings, escapes,
numbers, and source locations are easy to get wrong, and a separate parser
would let the published schema drift away from the types.

## Rationale

These libraries are necessary for a strict JSON format and a schema that stays
aligned with the types. Each is permissively licensed, currently maintained,
and published on crates.io. `schemars` stays optional because only schema
generation needs it. The tools that read and validate a project file do not.
