# ADR 0018: Finding lifecycle

- Status: Accepted
- Date: 2026-11-15

## Decision

A finding has one of three states: `open`, `ignored`, or `resolved`. Its
identity is the project id, the ecosystem, the package name, and the
vulnerability id. The version is remembered but is not part of the
identity, so a new affected version is the same finding.

A successful radar check inserts a new finding as `open`, refreshes a
finding it sees again, marks a `resolved` finding `open` when it comes
back, and marks a finding it no longer sees as `resolved`. An ignored
finding whose last ignore day is before the UTC day of the check becomes
`open`. An ignore with no end day stays ignored. Each of those changes
appends an event: `discovered`, `reappeared`, `resolved`, or `reopened`.
`findings ignore` and `findings reopen` append `ignored` and `reopened`.

A check that fails for a project does not change that project's findings.
The run is still recorded. A project with no lockfiles is the same for the
findings: they are left untouched. That run is recorded with `evaluated` 0
and `error` null, so it is not stored as a failure. `radar` exits 3 only
when a finding is `open` after the check, including a finding that stayed
open because no lockfiles were configured. Ignored and resolved findings
do not.

## Rationale

The user needs a stable id to ignore a finding, including after the package
moves to another affected version. A finding that the lockfile no longer
contains is resolved without a separate command. Exit status 3 stays the
signal that something still needs attention. An ignored finding is not that
signal. A failed check must not mark findings resolved, because the absence
of results is not evidence that the vulnerabilities are gone. Removing
every lockfile is the same kind of absence. `evaluated` records that
distinction without putting a sentence into `error`, which is reserved
for a check that could not be completed.
