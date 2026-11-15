# ADR 0019: Advisory kinds and exit codes

- Status: Accepted
- Date: 2026-11-15

## Decision

Every stored finding has a kind: `vulnerability`, `malicious`,
`unmaintained`, `unsound`, `notice`, `eol`, or `eol_soon`. An OSV id that
starts with `MAL-`, or a RustSec record whose affected
`database_specific.categories` contains `malicious`, is `malicious` and its
severity is `critical`. RustSec `informational` is read from
`affected[].database_specific`, then from the top-level
`database_specific`. `unmaintained`, `unsound`, and `notice` are kept. Any
other informational string is `notice`. A record with none of those marks
is a `vulnerability`.

A record with a non-empty top-level `withdrawn` timestamp is not shown and
is not inserted. A finding already stored for that id becomes `resolved`
with a `withdrawn` event.

`radar` lists `vulnerability`, `malicious`, and `eol` first, then
`unmaintained`, `unsound`, `notice`, and `eol_soon` under notices. Exit
status 3 is used when an open finding is `vulnerability`, `malicious`, or
`eol`. The notice kinds and `eol_soon` count only with `--strict`. A
project error still exits 1 and takes priority.

## Rationale

RustSec publishes unmaintained, unsound, and notice records through the
same OSV export as vulnerabilities, and marks malicious crates with a
category rather than a `MAL-` id. Treating every record as a vulnerability
would make an unmaintained dependency fail the same way as a known hole.
Withdrawn records are no longer claims, so they leave the open list
through their own event instead of being stored again. Exit status 3 stays
the signal that something still needs attention. Notices and a support end
that has not arrived yet are visible without failing the run, unless
`--strict` is set.
