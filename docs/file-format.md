# craftward.json

Each project is described by one `craftward.json` file. This document specifies
schema version 1. Field names are English and use `snake_case`. A reader
rejects any field that is not listed here.

The checked-in JSON Schema at [`schema/craftward.schema.json`](../schema/craftward.schema.json)
describes the types, enumerations, required fields, and the rejection of
unknown fields. The Rust validator in `craftward-core` is the full ruleset. The
schema does not express unique ids, calendar dates, URL prefixes, or relative
paths.

A JSON `null` in an optional field is treated as if the field had been omitted.
A required field must be present and must not be `null`.

Every error names a path and a cause. The path of the document itself, when the
root value is not an object, is `$`. Nested fields use a dotted path with
array indexes, for example `plan.phases[1].tasks[0].due`.

## Short names

Every `id` uses the same short-name rule:

- 1 to 64 characters
- only `a-z`, `0-9`, and `-`
- does not start or end with `-`

`a--b` is valid. `Harbor`, `-a`, and `a-` are not.

Phase ids are unique in the file. Task ids are unique within their phase, so
two phases may reuse a task id. Issue ids are unique in `issues`. Watch-item
ids are unique in `watch_items`. Ids in different lists do not have to differ
from each other.

## Names and titles

`name` and every `title` must contain a character that is not whitespace.
Other strings, including `label`, `description`, `goal`, `note`, and
`current_version`, may be empty.

## Dates

`updated`, task `due`, and watch-item `due` are strings in the form
`YYYY-MM-DD`. The year, month, and day must be a real Gregorian date, including
leap days. `2024-02-29` is valid. `2026-02-30` and `1900-02-29` are not.

## Paths

A lockfile path is relative to the project file. It is rejected when it is
empty, when it starts with `/` or `\`, when it starts with an ASCII letter
followed by `:`, or when any `/` or `\` segment is `..`. `foo..bar` is allowed,
because `..` is a segment and not a substring.

## Project

| Field | Required | Meaning |
| --- | --- | --- |
| `schema_version` | yes | Integer `1`. An integer greater than 1 means a newer Craftward wrote the file. |
| `id` | yes | Short name of the project. |
| `name` | yes | Human-readable name. |
| `status` | yes | Lifecycle status. |
| `description` | no | Longer description. |
| `current_version` | no | Version the maintainer currently ships or is working on. |
| `tags` | no | List of strings. Exact duplicates are rejected. |
| `updated` | no | Day the file was last updated. |
| `links` | no | List of labeled web addresses. |
| `stack` | no | Components and lockfiles. |
| `plan` | no | Phases and tasks. |
| `issues` | no | Bugs and other tracked problems. |
| `watch_items` | no | Dated reminders. |

`status` is one of `idea`, `in_development`, `released`, `maintenance`,
`paused`, `discontinued`.

```json
{
  "schema_version": 1,
  "id": "harbor-light",
  "name": "Harbor Light",
  "status": "idea"
}
```

When `schema_version` is an integer greater than 1, loading stops. The only
error is `file was written by a newer Craftward version`. Other fields in that
file are not checked. A value that is not an integer, including `1.0` or
`"1"`, is a validation error (`schema_version` must be 1) rather than a newer
file.

## links

Each item is an object with `label` and `url`. `url` must start with `http://`
or `https://`.

```json
{ "label": "Repository", "url": "https://example.com/harbor-light" }
```

## stack

When `stack` is present, `components` and `lockfiles` are both required. Either
list may be empty.

A component has:

| Field | Required | Meaning |
| --- | --- | --- |
| `kind` | yes | `platform`, `language`, `framework`, `runtime`, `sdk`, `service`, `tool`, or `other`. |
| `name` | yes | Component name. |
| `version` | no | Version the project uses. |
| `note` | no | How the component is used. |

```json
{
  "components": [
    { "kind": "language", "name": "Rust", "version": "1.100.0" }
  ],
  "lockfiles": ["Cargo.lock"]
}
```

## plan

When `plan` is present, `phases` is required.

A phase has `id`, `title`, optional `goal`, and `tasks`.

A task has:

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Short name, unique within the phase. |
| `title` | yes | Task title. |
| `status` | yes | `open`, `in_progress`, `done`, or `dropped`. |
| `due` | no | Due day. |
| `note` | no | Free-form note. |

```json
{
  "id": "foundation",
  "title": "Foundation",
  "goal": "Ship a file format the tools can trust.",
  "tasks": [
    {
      "id": "schema",
      "title": "Write the project file schema",
      "status": "done",
      "due": "2026-11-14"
    }
  ]
}
```

## issues

Each issue has:

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Short name, unique among issues. |
| `title` | yes | Issue title. |
| `severity` | yes | `low`, `medium`, `high`, or `critical`. |
| `status` | yes | `open`, `in_progress`, `resolved`, or `wont_fix`. |
| `url` | no | Tracker address. It must start with `http://` or `https://`. |
| `note` | no | Free-form note. |

```json
{
  "id": "theme-contrast",
  "title": "Status colors fail on a light background",
  "severity": "medium",
  "status": "open",
  "url": "https://example.com/harbor-light/issues/12"
}
```

## watch_items

Each reminder has:

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Short name, unique among watch items. |
| `kind` | yes | `deadline`, `expiry`, `review`, or `other`. |
| `title` | yes | Reminder title. |
| `due` | yes | The day this reminder is about. |
| `note` | no | Free-form note. |

```json
{
  "id": "domain-renewal",
  "kind": "expiry",
  "title": "Renew the example domain",
  "due": "2027-06-30"
}
```

## Changing the schema later

Adding, removing, or renaming a field, or changing the type or the allowed
values of a field, is a breaking change. It needs a new `schema_version`.

A build that implements version 1 accepts only the integer `1`. It does not
guess at the meaning of a newer file, and this version does not migrate files
forward or backward. The schema file
`schema/craftward.schema.json` is regenerated in the same change that changes
the Rust types:

```console
cargo run -p craftward-core --features schema --example generate-schema
```

`schemars` is not linked into the command-line tool or the desktop app. It is
an optional dependency of `craftward-core`, enabled by the `schema` feature,
and that feature is not a default feature. The generator and the comparison
test require it. A normal build skips both.
