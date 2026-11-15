# Vulnerability radar

`craftward radar` reads the lockfiles named by a project's `stack.lockfiles`
and asks [OSV](https://osv.dev/) which of those packages have a known
vulnerability. Each successful check is stored in the local database
described in [storage.md](storage.md). `craftward findings` reads that
database and does not use the network.

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

## Advisory kinds

Every OSV record is one of `vulnerability`, `malicious`, `unmaintained`,
`unsound`, or `notice`.

- An id that starts with `MAL-`, or an `affected` entry whose
  `database_specific.categories` contains `malicious` (any letter case),
  is `malicious`. Its severity is always `critical`.
- Otherwise the kind is the first
  `affected[].database_specific.informational` string, or the same field
  on the top-level `database_specific` object when no affected entry has
  one. `unmaintained`, `unsound`, and `notice` are kept. Any other
  non-empty string is stored as `notice`.
- A record with none of those marks is a `vulnerability`. Severity still
  comes only from the top-level `database_specific.severity`.

A record with a non-empty top-level `withdrawn` timestamp is not shown and
is not inserted. A finding already stored for that id becomes `resolved`
and gains a `withdrawn` event. That is separate from a finding that
disappears because the package is gone, which still uses `resolved`.

## End of support

A component that has a `version` is also checked against
[endoflife.date](https://endoflife.date) API v1. The product list is
`GET https://endoflife.date/api/v1/products/`. One product, including its
cycles, is `GET https://endoflife.date/api/v1/products/{name}/`. The path
contains only the product name. No version, project id, or path is sent.
The API specification used here is version 1.2.1. The data is under the
MIT License: <https://endoflife.date>.

The product is chosen in this order:

1. `eol_product` set to `none` skips that component. There is no warning
   and no request. Any letter case is accepted. `stack set` stores it
   with `--eol-product none`.
2. Otherwise `eol_product`, matched to a product name without regard to
   letter case.
3. Otherwise the component name, reduced to ASCII letters and digits and
   lower case, when that value matches exactly one product name or alias
   reduced the same way.
4. Otherwise a warning, `no endoflife.date product for <name>; set eol_product`.
   That is not an error.

The cycle is the longest one whose dot-separated parts are a prefix of the
version's parts. `9.3.0` matches cycle `9.3` before cycle `9`. Parts are
compared as text, so `3.1` does not match `3.12`. Two cycles of that same
length are a warning and produce no finding. No matching cycle is a
warning, `no endoflife.date cycle for <product> <version>`, unless the
version is numerically higher than every cycle name that is a dotted
number. That warning is `<name> <version> is newer than any cycle known to
endoflife.date`, and it produces no finding.

A matched cycle uses `isEol` and `eolFrom` from the product response.
`isEol` says whether the cycle has reached end of life. `eolFrom` is the
date, or null when the date is not known.

| `isEol` | `eolFrom` | Result |
| --- | --- | --- |
| true | a past date | `eol`, severity `high`, summary `support ended on <date>` |
| true | null | `eol`, severity `high`, summary `support ended on unknown` |
| false | today or within `--eol-warning-days` (default 90) | `eol_soon`, severity `medium` |
| false | a later date | no finding |
| false | null | no finding and no warning |

A warning is reserved for a cycle whose fields disagree or cannot be read:
`isEol` true with a date that is not in the past, `isEol` false with a date
in the past, a missing `isEol` or `eolFrom`, or an `eolFrom` that is not a
calendar day. The finding id is `EOL:<product>:<cycle>`. The ecosystem is
`endoflife` and the package name is the product. The link is
`https://endoflife.date/<product>`.

These findings use the same database, ignore, and resolve rules as OSV
findings. A component that moves to a cycle this check does not report
resolves the previous end-of-life finding. An end-of-life check does not
resolve OSV findings, and a lockfile check does not resolve end-of-life
findings. A component with no version is skipped.

The product list and each product are cached for 24 hours under `eol/` in
the cache directory. `--refresh` and `--offline` follow the same rules as
OSV. A network failure with a cached copy is a warning. No cache and no
network fails that project and changes nothing.

When an end-of-life check ran, the text report includes
`endoflife.date data, MIT License, https://endoflife.date`.

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

## Lifecycle

A finding is one project, one ecosystem, one package name, and one
vulnerability id. The version is not part of that identity. A later scan
that still affects the package updates the stored version and keeps the
same id.

On a successful check, in one transaction:

- A finding that is not stored yet is inserted as `open`, with a
  `discovered` event.
- A stored finding is updated. If it was `resolved`, it becomes `open`
  again and gains a `reappeared` event.
- An `ignored` finding whose `ignore_until` is a date before today (UTC)
  becomes `open` and gains a `reopened` event. The `until` day itself is
  still ignored. An ignore without `until` does not expire.
- A finding whose advisory this check saw as withdrawn becomes
  `resolved` and gains a `withdrawn` event. It does not also gain
  `resolved`.
- A finding that this check did not see, and that is not already
  `resolved`, becomes `resolved`, but only inside the ecosystems this
  check actually compared. Lockfile ecosystems are compared only when
  `stack.lockfiles` is not empty. `endoflife` is compared only when a
  component version was checked. An ignored finding that disappeared
  from a compared ecosystem is included.
- The run is appended, including how many packages were checked and how
  many findings are open, new, or newly resolved.

A failed check (the lockfile cannot be read, or there is no network and no
cache) does not change that project's findings. The run is still appended,
with `error` set and the counters at zero.

A project with no lockfiles and no versioned component is not a check.
Its findings stay as they are, including open and ignored ones. The run
is appended with `evaluated` 0 and `error` null. `open_count` is how many
findings were already open. The other counters are zero. `error` stays
empty because nothing failed: there was nothing to compare. A project
with no lockfiles but a versioned component still checks end of support.
That run has `evaluated` 1. Its OSV findings stay until lockfiles are
configured again.

`findings ignore` stores the reason and an optional last day. `findings
reopen` makes the finding `open` and clears the ignore. Reopening a
finding that is already open is an error.

## Text report

Each project starts with its id. Each lockfile that was read is one line,
`lockfile <path>: queried N, skipped M`. Warnings and errors are indented
under the project. A project with an empty lockfile list and no warning or
error prints `no lockfiles configured`. When that project still has an
open or ignored finding that did not come from endoflife.date, the report
adds `existing findings are unchanged until lockfiles are configured` and
still lists the open findings. An empty workspace prints `no projects`.

Open findings of kind `vulnerability`, `malicious`, and `eol` are listed
first. `unmaintained`, `unsound`, `notice`, and `eol_soon` follow under a
`notices:` heading. Each group is ordered by severity (`critical`, `high`,
`medium`, `low`, `unknown`), then by package, version, and vulnerability
id. A finding inserted by this run is marked `new`. Each one prints the
package and version, `#` and the database id, the vulnerability id,
aliases in parentheses when there are any, the severity, the kind, the
summary, `fixed in` when fixed versions were recorded, and the link.

Ignored findings are not listed unless `--all` is set. The report always
prints `ignored: N` and `resolved: N` for a successful check. `N` for
ignored is how many ignored findings were still present. `N` for resolved
is how many this run marked resolved. When all three are zero, the report
also prints `no open findings`.

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
          "record_id": 1,
          "new": true,
          "state": "open",
          "ecosystem": "crates.io",
          "package": "example",
          "version": "1.0.0",
          "id": "RUSTSEC-2024-0001",
          "aliases": ["CVE-2024-1"],
          "severity": "high",
          "kind": "vulnerability",
          "summary": "Short description",
          "fixed_versions": ["1.2.0"],
          "url": "https://osv.dev/vulnerability/RUSTSEC-2024-0001"
        }
      ],
      "ignored": 0,
      "resolved": 0,
      "evaluated": true,
      "warnings": [],
      "error": null,
      "stale_cache_from": null
    }
  ]
}
```

`id` inside a finding is the OSV id. `record_id` is the database id used
by `findings`. Without `--all`, `findings` lists only open findings.
`--all` appends the ignored ones. `ignored` and `resolved` are the same
counts as in the text report. `kind` is the advisory kind. `evaluated` is
false when neither lockfiles nor end of support were compared, or the
check failed. `error` is a string only when the check
failed, and null when nothing was evaluated. `stale_cache_from` is
`YYYY-MM-DD HH:MM:SS UTC` when a network failure was answered from the cache.

`findings --json` prints `{ "findings": [ ... ] }`. Each object uses `id`
for the database id and `vulnerability` for the OSV id, plus `project_id`,
`kind`, `state`, `first_seen`, `last_seen`, `resolved_at`, `ignore_reason`,
and `ignore_until`. `findings show --json` prints `{ "finding": { ... },
"events": [ { "kind": "discovered", "at": "YYYY-MM-DDTHH:MM:SSZ", "reason": "" } ] }`.
Event kinds are `discovered`, `ignored`, `reopened`, `resolved`,
`reappeared`, and `withdrawn`.

## Exit status

| Status | Meaning |
| --- | --- |
| 0 | No actionable finding is open. |
| 1 | A project could not be checked, or the cache or data directory could not be chosen. |
| 2 | The command line is wrong. |
| 3 | An open finding is a `vulnerability`, `malicious`, or `eol`. With `--strict`, an open `unmaintained`, `unsound`, `notice`, or `eol_soon` also counts. |

Status 1 wins over status 3. A project with no lockfiles still exits 3 when
one of its stored actionable findings is open. Notices and `eol_soon` do
not, unless `--strict` is set. `findings` does not use status 3.
