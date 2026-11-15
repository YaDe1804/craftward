# Vulnerability radar

`craftward radar` reads the lockfiles named by a project's `stack.lockfiles`
and asks [OSV](https://osv.dev/) which of those packages have a known
vulnerability. It does not store findings in a database and it does not
mark anything done.

## What is read

Lockfile paths are resolved from `source_dir`, or from the project file's
directory when `source_dir` is absent. The file name selects the reader.
The comparison is case-insensitive.

`Cargo.lock` format versions 3 and 4 are read. Each `[[package]]` entry
needs a name and a version. A package is queried only when `source` refers
to the crates.io registry (`github.com/rust-lang/crates.io-index` or
`index.crates.io`). A package with no source, a git source, or another
registry is skipped and counted. Any other format version is an error that
names the version and the supported ones.

`package-lock.json` versions 2 and 3 are read from the `packages` object.
The empty root key and entries with `"link": true` are omitted and are not
counted as skipped. The package name is the `name` field when it is a
non-empty string. Otherwise it is the path section after the last
`node_modules` segment, including an `@scope` when the next segment starts
with `@`. The same name and version are queried once. Version 1 is an
error that says it is not supported.

Any other file name is a warning. The project is still checked. A lockfile
that is missing, not valid UTF-8, or not valid for its format is an error
for that project. The message includes a line number when the reader knows
one. Other projects in the same run are still checked.

## What is sent

Each query is one ecosystem, one package name, and one version. Craftward
sends nothing else: no paths, no project id, and no source text.

The ecosystems are the OSV names `crates.io` and `npm`. Queries go to
`POST https://api.osv.dev/v1/querybatch` in groups of at most 1000, which
is the maximum the OSV API accepts per batch. If a query has more pages,
the next request sends only the queries that returned `next_page_token`.
Each distinct vulnerability id is then loaded once from
`GET https://api.osv.dev/v1/vulns/{id}`.

From that record Craftward keeps the id, the aliases, the summary, the
affected packages, and the severity. Severity is
`database_specific.severity` when the value is `LOW`, `MODERATE`, `HIGH`,
or `CRITICAL` (any letter case), mapped to `low`, `medium`, `high`, and
`critical`. Otherwise the severity is `unknown`. Craftward does not score
CVSS itself. Fixed versions are the `fixed` events on ranges of type
`ECOSYSTEM` or `SEMVER`. Git ranges are ignored because their events are
commits. crates.io records usually use `SEMVER`, so both types are kept.

## Cache

Results are JSON files. The directory is `--cache-dir`, then
`CRAFTWARD_CACHE_DIR`, then:

| System | Default |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Craftward\cache` |
| Linux | `$XDG_CACHE_HOME/craftward`, or `~/.cache/craftward` |
| macOS | `~/Library/Caches/Craftward` |

An empty environment variable is treated as unset.

A query result, one file per ecosystem, name, and version, is fresh for 24
hours. A vulnerability record is fresh for 7 days. Files are replaced by
writing a temporary file and renaming it. `--refresh` does not read a fresh
entry. `--offline` never opens a connection. `--refresh` and `--offline`
together are a usage error.

If the network fails and a cached record exists, that record is used even
when it is older than its lifetime. The report says so and includes the
time it was stored. If there is no cached record, that project fails and
the other projects continue. A failure while writing the cache does not
fail the project.

`--offline` with an expired record also names the stored time. It does not
describe that as a network error, because no request was made.

## Text report

Each project starts with its id. Each lockfile that was read is one line,
`lockfile <path>: queried N, skipped M`. Warnings and errors are indented
under the project. A project with an empty lockfile list and no warning or
error prints `no lockfiles configured`. A checked project with no
vulnerabilities prints `no findings`. An empty workspace prints
`no projects`.

Findings are ordered by severity (`critical`, `high`, `medium`, `low`,
`unknown`), then by package, version, and id. Each finding prints the
package and version, the vulnerability id, aliases in parentheses when
there are any, the severity, the summary, `fixed in` when fixed versions
were recorded, and `https://osv.dev/vulnerability/<id>`.

## JSON report

`--json` prints one JSON document on the standard output. The text report
is not also printed. Usage errors are still a line on the standard error
stream and produce no JSON document.

```json
{
  "projects": [
    {
      "id": "harbor-light",
      "path": "app/craftward.json",
      "lockfiles": [
        { "path": "Cargo.lock", "kind": "cargo", "queried": 1, "skipped": 0 }
      ],
      "findings": [
        {
          "ecosystem": "crates.io",
          "package": "example",
          "version": "1.0.0",
          "id": "RUSTSEC-2024-0001",
          "aliases": ["CVE-2024-1"],
          "severity": "high",
          "summary": "Short description",
          "fixed_versions": ["1.2.0"],
          "url": "https://osv.dev/vulnerability/RUSTSEC-2024-0001"
        }
      ],
      "warnings": [],
      "error": null,
      "stale_cache_from": null
    }
  ]
}
```

`kind` is `cargo` or `npm`. `error` is a string when that project could not
be checked, and null otherwise. `stale_cache_from` is `YYYY-MM-DD HH:MM:SS UTC`
when a network failure was answered from the cache, and null otherwise.
Field names and this nesting are stable.

## Exit status

| Status | Meaning |
| --- | --- |
| 0 | No vulnerability was found. |
| 1 | A project could not be checked, or the cache directory could not be chosen. |
| 2 | The command line is wrong. |
| 3 | At least one vulnerability was found. |

Status 1 wins over status 3.
