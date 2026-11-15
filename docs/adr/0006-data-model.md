# ADR 0006: Data model

- Status: Accepted
- Date: 2026-11-14

## Decision

One `craftward.json` per project is the source of truth for user-maintained
data. A local SQLite database holds app-generated state (radar findings, cache,
history). Storage sits behind an interface.

## Rationale

Storage sits behind an interface so a team server can be added later.
