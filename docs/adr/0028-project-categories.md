# ADR 0028: Project categories

- Status: Accepted
- Date: 2026-11-17

## Decision

Each project may have one optional free-text `category` field for portfolio
grouping. Tags stay cross-cutting. Platforms are not stored as categories;
they are derived from the stack (`stack.android` and components with kind
`platform`), merged without regard to letter case, and shown with readable
labels.

Unknown JSON fields are rejected. Therefore Craftward writes
`schema_version` `2` only when `category` is present, and writes `1` when it
is absent (including after `--clear-category`). Load accepts versions `1`
and `2`. A version-1 file must not contain `category`. An integer greater
than `2` remains `UnsupportedSchema`.

Category text is normalized before compare and store: trim, collapse
internal whitespace to single spaces, require 1–40 characters, reject
control characters. When saving against existing portfolio spellings, a
case-insensitive match reuses the existing spelling.

## Rationale

Putting `category` on version 1 would make older checkouts fail with an
unknown-field error instead of the clear “newer Craftward version” message.
Writing version 2 only when the field is used keeps category-free files
readable by older builds. One optional string keeps the sidebar simple;
platforms stay filters so Android is not duplicated as both a component
name and a category.
