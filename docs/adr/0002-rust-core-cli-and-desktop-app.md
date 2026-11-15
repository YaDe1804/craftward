# ADR 0002: Rust, split into core library, CLI, and desktop app

- Status: Accepted
- Date: 2026-11-14

## Decision

Implement Craftward in Rust, split into a core library, a command-line
interface, and a desktop app.

## Rationale

The core is usable without a UI and the UI stays replaceable.
