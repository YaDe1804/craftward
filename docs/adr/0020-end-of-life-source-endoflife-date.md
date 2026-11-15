# ADR 0020: End-of-life source endoflife.date

- Status: Accepted
- Date: 2026-11-15

## Decision

Support ends are read from the endoflife.date API v1, specification 1.2.1.
The product list is `GET /api/v1/products/`. One product and its cycles are
`GET /api/v1/products/{name}/`. The request path contains only the product
name. The data is under the MIT License. A report that used it says so and
links to <https://endoflife.date>.

`craftward.json` stays at schema version 1. A component may set
`eol_product`. The value `none` skips that component. A component is
checked only when it has a version and is not skipped. The product is
`eol_product` when set, otherwise the component name when that name
matches exactly one product or alias after reducing both to ASCII letters
and digits in lower case. Anything else is a warning that names the
component and tells the user to set `eol_product`. It is not an error.

The cycle is the longest one whose dot-separated parts are a prefix of the
version parts. Two cycles of that length are a warning and produce no
finding. A version numerically higher than every numeric cycle name is a
warning that the version is newer than any known cycle, and produces no
finding. `isEol` true with a past `eolFrom`, or with a null `eolFrom`, is
kind `eol`, severity `high`. `isEol` false with `eolFrom` today or within
`--eol-warning-days` (default 90) is kind `eol_soon`, severity `medium`.
`isEol` false with a null or later `eolFrom` produces nothing. Disagreeing
or unreadable fields are a warning. The finding id is `EOL:<product>:<cycle>`,
the ecosystem is `endoflife`, and the package name is the product. The
link is the product page. These findings share the finding lifecycle. The
product list and each product are cached for 24 hours, with the same
`--refresh` and `--offline` rules as OSV.

## Rationale

A lockfile does not say when a runtime or a database leaves support. The
component list already records those names and versions, and an optional
product id covers names that are not unique. Sending only the product name
keeps project paths and versions off that request. The longest prefix
selects `9.3` for `9.3.0` without treating a nearby cycle as the same
release. A missing product or cycle is incomplete data, not a failed
check. A support end that has already passed is the same kind of attention
as an open vulnerability. One that is only approaching is a notice unless
`--strict` is set. The MIT License requires the report and the
documentation to name the source.
