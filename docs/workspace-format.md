# workspace.json

A workspace file remembers where each project's `craftward.json` lives. It does
not copy the project. Field names are English `snake_case`. Unknown fields are
rejected. This document specifies schema version 1.

## Where the file is found

Craftward looks in this order:

1. The `--workspace <FILE>` option.
2. The `CRAFTWARD_WORKSPACE` environment variable, when it is set and not empty.
3. A suggested directory for the operating system, in a file named
   `workspace.json`:
   - Windows: `%APPDATA%\Craftward\workspace.json`
   - Linux: `$XDG_CONFIG_HOME/craftward/workspace.json` when
     `XDG_CONFIG_HOME` is set and not empty, otherwise
     `~/.config/craftward/workspace.json`
   - macOS: `~/Library/Application Support/Craftward/workspace.json`

Other Unix systems use the Linux rule. The home directory comes from
`std::env::home_dir`. Windows configuration uses `APPDATA` and does not use
that function.

An empty `CRAFTWARD_WORKSPACE` or `XDG_CONFIG_HOME` is treated as unset. A
relative `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, or `XDG_DATA_HOME` is ignored
and the home-directory default is used instead. A relative path from the
option or `CRAFTWARD_WORKSPACE` is resolved from the current directory. The
suggested directories are defaults only. `--workspace` and
`CRAFTWARD_WORKSPACE` replace them.

A missing workspace file means there are no projects yet. The first successful
`craftward add` creates it.

## Fields

| Field | Required | Meaning |
| --- | --- | --- |
| `schema_version` | yes | Integer `1`. |
| `projects` | yes | List of project files. It may be empty. |

Each project entry has one field:

| Field | Required | Meaning |
| --- | --- | --- |
| `path` | yes | Path of a `craftward.json` file. It must not be empty. |

A relative `path` is resolved from the directory that contains
`workspace.json`. Craftward writes `/` between the components of a relative
path. An absolute path is a normal absolute path for the operating system,
produced with `std::path::absolute`. On Windows that looks like
`C:\dir\craftward.json`. The `\\?\` prefix added by `std::fs::canonicalize`
is never written. Canonical paths are used only to tell whether two entries
are the same file. Duplicate path strings, after `\` is treated as `/`, are
rejected.

```json
{
  "schema_version": 1,
  "projects": [
    {
      "path": "harbor-light/craftward.json"
    }
  ]
}
```

When `schema_version` is an integer greater than 1, loading stops. The only
error is `file was written by a newer Craftward version`. Other fields are not
checked. A value that is not an integer is a validation error.

## Writing

Updates are written as pretty JSON, with LF line endings and a final newline.
The new text is written to a temporary file in the same directory and then
renamed over `workspace.json`. Parent directories are created when they are
missing. A failed write deletes the temporary file.
