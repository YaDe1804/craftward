# ADR 0029: Merging aliased advisories and severity

- Status: Accepted
- Date: 2026-11-17

## Decision

Radar findings that share ecosystem, package, and version and are linked
through OSV ids or aliases are merged into one finding (transitive
union-find). The primary id prefers GHSA, then RUSTSEC, then the first
sorted id; other ids stay on the row as `related_ids` / aliases. Kind is
the most severe of the cluster; fixed versions are the union.

Severity prefers an advisory qualitative label (`database_specific.severity`).
Otherwise a CVSS v3.0/v3.1 vector is scored locally (FIRST base metrics) and
mapped to critical/high/medium/low. CVSS v4 contributes only when a
qualitative label is present. Otherwise severity is unknown. JSON reports
include `severity_source`: `advisory`, `cvss`, or `unknown`.

Schema version 7 stores `related_ids` and `severity_source`, and a
`finding_merges` table. Opening an older database backs up, adds the
columns, merges existing duplicate rows (oldest survivor; ignore state
kept when any member was ignored), and records redirects so
`findings show <old-id>` reports the surviving id.

Portfolio order (app and `craftward list`) uses a single core ranking:
project status (released → maintenance → in_development → paused → idea →
discontinued), then open-finding severity counts, then next due, then name.

## Rationale

OSV often returns both GHSA and RUSTSEC records for the same issue. Showing
them twice inflated open counts (SQ-H1). Merging on aliases matches how
operators read advisories. Local CVSS scoring avoids leaving GHSA-only
vectors as unknown. Status-first sorting matches how maintainers scan a
portfolio of shipped vs. unfinished work.
