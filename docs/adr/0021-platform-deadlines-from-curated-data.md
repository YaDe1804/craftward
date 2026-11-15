# ADR 0021: Platform deadlines from curated data

- Status: Accepted
- Date: 2026-11-15

## Decision

Google Play target-API deadlines live in `data/platform-deadlines.json`
and are compiled into the program. Radar does not fetch them. Each row
records the Play Console Help URL it was copied from and the day that page
was read. A row is added only when that page states the level and the
date. A `data_version` more than 365 days old produces a warning and is
still used. A valid `platform-deadlines.json` in the data directory
replaces the built-in file when its `data_version` is newer. An older or
invalid file does not.

`craftward.json` stays at schema version 1. `stack.android` is optional.
`target_sdk`, when set, is the level that is compared. Otherwise Craftward
reads the usual `targetSdk` and `targetSdkVersion` assignments in
`detect_from`, and resolves `libs.versions.<key>` from the `[versions]`
table of a listed TOML file with its own reader. Comments are ignored.
Assignments inside `testOptions`, `lint`, and `managedDevices`, and text
inside strings, are ignored. The lowest value wins, and the other
locations are named, so a higher test or lint level cannot hide a lower
production level. An expression that cannot be read is a warning and
produces no finding. `min_sdk` is stored and is not compared. `store`
`none`, and a project with no `stack.android`, skip the check.

For one form factor and one requirement, the governing past row is the
newest whose deadline is today or earlier. Older rows stay in the file so
the page can still be checked against them, and they are not reported.
When that row is unmet and `targetSdk` is below it, the finding is
`platform_deadline`, severity `high`, and counts toward exit status 3.
When it is met, or there is no past row, the next future row within
`--deadline-warning-days` (default 180) is `platform_deadline_soon`,
severity `medium`, and counts only with `--strict`. The finding id is
`PLAY:<form factor>:<requirement>`. The required level, the deadline, and
the kind are written onto that same finding. A change from
`platform_deadline_soon` to `platform_deadline` records `deadline_passed`.
The ecosystem is `platform` and the package name is `google-play`. The
link is the governing row's source URL. The finding is resolved only when
a later check sees a `targetSdk` that meets the row that currently
governs that id. Schema version 4 added the two finding kinds. Schema
version 5 adds `deadline_passed` to the event list.

## Rationale

Google publishes the requirements as a help page and does not offer a
feed. Downloading the page would need the network, would depend on the
page layout, and could not be tested offline. A file in the repository
can be reviewed, and the one-year warning shows when nobody has checked
it. The Gradle forms projects write by hand are a small set. Running
Gradle, or taking a TOML library, would add a dependency and still miss
values that come from a plugin. An explicit `target_sdk` covers those.
The deadline day counts as reached because the page states the day the
requirement is in force, not the day after. Device families the page
states separately, including Android XR, are separate rows so a phone
rule is not applied to a watch, a TV, a car, or a headset. One finding per
form factor and requirement keeps a raised API level on the same record.
Reporting every past row would leave a finding open after the page had
replaced it. The id has no date, so the update does not look like a new
finding, and `deadline_passed` records the day the warning became a
missed deadline.
