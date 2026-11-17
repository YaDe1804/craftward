# Command-line interface

`craftward` reads and checks project files and keeps a list of them in a
[workspace file](workspace-format.md). With no arguments it prints its name and
version.

Exit status is `0` for success, `1` when a file or request fails, and `2` when
the command line is wrong. `radar` also exits `3` when an actionable
finding is open. Notices, a support end that is only approaching, and a
Play deadline that is only approaching count with `--strict`. An error
still exits `1`. Ignored findings do not change
that status.
`--help` after a command prints that command's help and exits `0`.

`--workspace <FILE>` selects the workspace file for that invocation. Otherwise
Craftward uses `CRAFTWARD_WORKSPACE`, then `settings.json`, then the default
location described in the workspace format. `--data-dir` and `--cache-dir`
follow the same order with their environment variables. Every path the
program prints uses the separator of the operating system.

## init

Creates `DIR/craftward.json` with only the required fields. `DIR` defaults to
the current directory. `--status` defaults to `idea`. An existing file is left
unchanged. The new project is not added to the workspace.

```console
craftward init --id harbor-light --name "Harbor Light"
craftward init ./app --id harbor-light --name "Harbor Light" --status in_development
```

## add

Adds a `craftward.json` file, or a directory that contains one, to the
workspace. The file must be valid. The same file and the same project id are
rejected. A relative path is stored relative to the workspace file when the
project lives inside that directory.

```console
craftward add ./app
craftward --workspace ./workspace.json add ./app/craftward.json
```

## remove

Removes one project id from the workspace. Without `--delete-file` the
project file stays on disk. `--delete-file` deletes only that project's
`craftward.json` after updating the workspace; the folder and every other
file are left alone.

```console
craftward remove harbor-light
craftward remove harbor-light --delete-file
```

## list

Prints one row per project:

| Column | Meaning |
| --- | --- |
| `ID` | Project id. |
| `NAME` | Project name. |
| `STATUS` | Lifecycle status. |
| `VERSION` | `current_version`, or `-` when it is absent. |
| `OPEN_TASKS` | Tasks with status `open` or `in_progress`. |
| `OPEN_ISSUES` | Issues with status `open` or `in_progress`. |
| `NEXT_DUE` | Earliest outstanding date, or `-`. |
| `NOTE` | Why a file could not be read, when it could not. |

The next due date is the earliest `tasks.due` among tasks that are `open` or
`in_progress`, together with every `watch_items.due`. Tasks marked `done` or
`dropped` are left out. A missing or invalid project file is still listed, with
the reason in `NOTE`, and the command exits `1`.

```console
craftward list
```

## validate

With paths, checks those files. Without paths, checks every project in the
workspace. Each file is reported as `ok` or as a list of errors. An error names
its location, for example `plan.phases[0].tasks[1].due`. A missing
`source_dir` or lockfile is printed as `warning:` on the standard error
stream. A warning does not change the exit status.

```console
craftward validate
craftward validate examples/minimal.craftward.json
```

## roadmap

Writes one HTML file for a project. The argument is a workspace project id,
or a `craftward.json` file, or a directory that contains one. A value is a
path when it exists, ends with `.json`, contains `/` or `\`, or is absolute.
Otherwise it is a project id. If it is both a registered project id and an
existing path, the command exits 2. Pass `.\name` on Windows or `./name` on
Linux and macOS to use the folder. Pass the path of the project file to use
the registered project.

`--dir` is the output directory and defaults to the current directory.
`--lang` is `en` or `de` and defaults to `en`. It selects the fixed labels.
Project titles and task text are copied as written.

The file name is `<id>_roadmap_vN.html`. `N` is one higher than the highest
existing number in that directory, or 1 when none exist. `v1` and `v3` produce
`v4`. An existing file is never overwritten. The command prints the path of
the new file.

```console
craftward roadmap harbor-light
craftward roadmap ./app/craftward.json --dir ./docs/roadmap --lang de
```

## workspace path

Prints the workspace file that this invocation would use, and whether that
choice came from the command-line option, `CRAFTWARD_WORKSPACE`, the
settings file, or the default location. The file does not have to exist
yet.

```console
craftward workspace path
craftward --workspace ./workspace.json workspace path
```

## settings

Shows or changes the storage locations stored in `settings.json`. Path
resolution and relocation rules are the same as in the desktop Settings
screen; see [storage.md](storage.md).

`settings show` prints the settings file path and each of workspace,
`data_dir`, and `cache_dir` with its origin.

`settings set` has one subcommand per location. The new path is a
positional argument. The global `--workspace`, `--data-dir`, and
`--cache-dir` options keep their usual meaning (override for this
invocation) and are never the destination. Existing workspace and
database files are copied, checked, then removed from the old path. The
cache directory is created empty. A target that already exists is
refused. A location fixed by a command-line option or environment
variable cannot be changed.

```console
craftward settings show
craftward settings set workspace D:/Craftward/workspace.json
craftward settings set data-dir D:/Craftward/data
craftward settings set cache-dir D:/Craftward/cache
```

## show

Prints one project so its ids can be copied into the commands below. The
header lists id, name, status, and the optional version, description,
source directory, update day, and tags. Each phase lists every task with
its id and status, including tasks that are done or dropped. Open issues
are those with status `open` or `in_progress`. Resolved and `wont_fix`
issues are omitted. `issues`, `watch_items`, and `stack` are always
printed; an empty list says `none`. The stack section lists components
(kind, name, optional version/note/eol_product), lockfile paths, and
Android fields when present.

`PROJECT` is a workspace project id, a `craftward.json` file, or a directory
that contains one. A value is a path when it exists, ends with `.json`,
contains `/` or `\`, or is absolute. Otherwise it is a project id. If it is
both a registered project id and an existing path, the command exits 2. Pass
`.\name` on Windows or `./name` on Linux and macOS to use the folder. Pass
the path of the project file to use the registered project. The same rule
applies to every command in this section.

```console
craftward show harbor-light
craftward show ./app/craftward.json
```

## project set

Changes header fields. At least one option is required. `--version ""`,
`--description ""`, `--source-dir ""`, and `--tags ""` remove those fields.
`--clear-tags` and `--clear-links` drop the whole list. `--status` is
`idea`, `in_development`, `released`, `maintenance`, `paused`, or
`discontinued`. `--tags` is a comma-separated list. `--link LABEL=URL` may
be repeated and replaces the whole links list.

A successful change rewrites the whole file: two-space indentation, LF line
endings, a trailing newline, the field order of the Rust types, and letters
such as `ä` left as characters. The first write may change the formatting of
a file that was edited by hand. `updated` is set to today's UTC date. The
file is replaced only when it is still valid. Otherwise it is left unchanged
and every error is printed.

An unknown status exits 2 and lists the allowed words. A value that breaks a
project rule, such as a blank name, exits 1 and leaves the file untouched.

```console
craftward project set harbor-light --name "Harbor Light" --status in_development
craftward project set harbor-light --version 0.5.0
craftward project set harbor-light --description ""
craftward project set harbor-light --tags "android,cli" --link Docs=https://example.com
```

## phase

`phase add` appends a phase and creates `plan` when the project has none.
`--title` is required. Without `--id`, the id is formed from the title:
letters are lowercased, `ä ö ü ß` become `ae oe ue ss`, other accented Latin
letters become their base letter when that fold is known, anything else
outside `a-z` and `0-9` becomes a hyphen, hyphens are collapsed and trimmed,
and the result is cut to 64 characters without a trailing hyphen. If that id
is already used by a phase, `-2`, `-3`, and so on are appended. A title that
yields no id is an error and names `--id`. The command prints the id it
stored.

`phase set` changes `--title` and/or `--goal` on an existing phase. At least
one of those options is required. An empty `--goal` clears the goal.

`phase remove` deletes a phase only when it has no tasks. Otherwise it exits
1 and names `--force`. `--force` removes the phase and its tasks.

An unknown phase id exits 1 and lists the phase ids in the file.

```console
craftward phase add harbor-light --title "Prüfung"
craftward phase set harbor-light pruefung --title "Prüfung" --goal "Ship it"
craftward phase remove harbor-light foundation
craftward phase remove harbor-light foundation --force
```

## task

`task add` appends a task to a phase. `--title` is required. The status
defaults to `open`. `--status` is `open`, `in_progress`, `done`, or
`dropped`. `--due` is `YYYY-MM-DD`. Without `--id`, the id is formed the
same way as a phase id and must be unique among that phase's tasks.

`task set` changes `--title`, `--due`, and/or `--note`. At least one of
those options is required. An empty `--due` or `--note` clears that field.

`task status` sets the status. `task done` is `task status` with status
`done`. `task remove` deletes one task.

An unknown phase or task exits 1 and lists the ids in that list. A bad
status or date exits 2 and lists the allowed values.

```console
craftward task add harbor-light foundation --title "Write the schema"
craftward task set harbor-light foundation schema --due 2026-12-01 --note "Draft"
craftward task status harbor-light foundation schema in_progress
craftward task done harbor-light foundation schema
craftward task remove harbor-light foundation schema
```

## issue

`issue add` appends an issue with status `open`. `--title` and `--severity`
are required. Severity is `low`, `medium`, `high`, or `critical`. `--url`
must start with `http://` or `https://`. Without `--id`, the id is formed
from the title and must be unique among issues.

`issue set` changes `--title`, `--severity`, `--url`, and/or `--note`. At
least one of those options is required. An empty `--url` or `--note` clears
that field.

`issue status` sets the status to `open`, `in_progress`, `resolved`, or
`wont_fix`. `issue remove` deletes one issue.

An unknown issue exits 1 and lists the issue ids. A bad status, severity, or
URL exits 2 and lists the allowed values.

```console
craftward issue add harbor-light --title Contrast --severity medium
craftward issue set harbor-light theme-contrast --severity high --note "WCAG"
craftward issue status harbor-light theme-contrast in_progress
craftward issue remove harbor-light theme-contrast
```

## watch

`watch add` appends a watch item. `--kind`, `--title`, and `--due` are
required. Kind is `deadline`, `expiry`, `review`, or `other`. `--due` is
`YYYY-MM-DD`. Without `--id`, the id is formed from the title and must be
unique among watch items.

`watch set` changes `--title`, `--kind`, `--due`, and/or `--note`. At
least one of those options is required. An empty `--note` clears that
field.

`watch remove` deletes one watch item. An unknown id exits 1 and lists the
watch ids. A bad kind or date exits 2 and lists the allowed values.

```console
craftward watch add harbor-light --kind expiry --title "Renew domain" --due 2027-06-30
craftward watch set harbor-light renew-domain --kind deadline --due 2027-07-01 --note ""
craftward watch remove harbor-light domain-renewal
```

## stack

Changes `stack` in a project file. The file is written only when the result
is still valid, and `updated` is set to the UTC date. A component is
addressed by `name`, without regard to letter case. Two components with
the same name are an error that says so.

`stack add` needs `--kind` and `--name`. Kind is `platform`, `language`,
`framework`, `runtime`, `sdk`, `service`, `tool`, or `other`. `--version`,
`--note`, and `--eol-product` are optional. The stack is created when the
project has none. A name that is already used exits 1.

`stack set` changes those optional fields on the one matching component.
An empty value removes the field. An unknown name exits 1 and lists the
component names.

`stack remove` deletes that component.

`stack lockfile add` appends a path. `stack lockfile remove` deletes one.
The path is compared exactly. An absolute path or a `..` segment is
rejected and the file is not written.

`stack android` creates or changes `stack.android`. `--target-sdk` and
`--min-sdk` take a positive integer. An empty value removes that field.
`--form-factor` is `phone`, `automotive`, `wear`, `tv`, or `xr`.
`--store` is `google_play` or `none`. `--detect-from` may be repeated and
appends a relative path. `--clear-detect-from` drops the current list
first. A bad form factor, store, or API level exits 2. An absolute path
or a `..` segment exits 1 and the file is not written.

Exit status is `0` on success, `1` when the name, path, or file is wrong,
and `2` when the command line is wrong. An unknown kind exits 2.

```console
craftward stack add harbor-light --kind runtime --name Node.js --version 22 --eol-product nodejs
craftward stack set harbor-light Node.js --version ""
craftward stack remove harbor-light Node.js
craftward stack lockfile add harbor-light package-lock.json
craftward stack lockfile remove harbor-light package-lock.json
craftward stack android harbor-light --target-sdk 35 --detect-from app/build.gradle.kts
```

## radar

Looks up known vulnerabilities for the packages in the lockfiles named by
`stack.lockfiles`. With no project, every project in the workspace is
checked. A project argument is an id or a path, with the same ambiguity
rule as `roadmap`.

`--refresh` ignores cached answers and asks again. `--offline` never
contacts OSV or endoflife.date. The two options cannot be combined.
`--strict` makes notices, `eol_soon`, and `platform_deadline_soon` count
toward exit status 3. `--eol-warning-days` is how many days ahead a
support end still counts as `eol_soon`. The default is 90.
`--deadline-warning-days` is how many days ahead a Play deadline still
counts as `platform_deadline_soon`. The default is 180. Play deadlines
do not use the network. `--cache-dir` selects the
cache directory; otherwise `CRAFTWARD_CACHE_DIR` is used, then the default
described in [radar.md](radar.md). `--data-dir` selects the database
directory; otherwise `CRAFTWARD_DATA_DIR` is used, then the default
described in [storage.md](storage.md). `--json` prints the same report as
JSON. `--all` includes ignored findings in the list. They are always counted.

Exit status is `0` when no actionable finding is open, `3` when a
vulnerability, a malicious package, a past support end, or a past Play
deadline is open, `1` when a project could not be checked, and `2` when
the command line is wrong. Notices, `eol_soon`, and
`platform_deadline_soon` count toward status 3 only with `--strict`.
An error takes priority over an open finding. The text
report, the JSON fields, the lifecycle, and what is sent to OSV are
described in [radar.md](radar.md).

```console
craftward radar
craftward radar harbor-light --json
craftward radar --offline --cache-dir ./cache --data-dir ./data
craftward radar --strict --eol-warning-days 30
craftward radar --deadline-warning-days 90
```

## deadlines

Prints the Play target-API deadlines that `radar` will use, their
`data_version`, and the source URL of each row. `--data-dir` selects the
directory that may hold a newer `platform-deadlines.json`. The built-in
file is used when that file is missing, older, or invalid. An older file
is named. An invalid file is a warning. The layout and the update steps
are in [platform-deadlines.md](platform-deadlines.md).

Exit status is `0` on success, `1` when the built-in file or the data
directory cannot be used, and `2` when the command line is wrong.

```console
craftward deadlines
craftward deadlines --data-dir ./data
```

## findings

Reads the findings stored by `radar`. It does not contact OSV. With no
project, every stored finding is listed. A project argument is an id or a
path, with the same ambiguity rule as `roadmap`. `--state` is `open`,
`ignored`, `resolved`, or `all`. The default is `open`. `--kind` limits
the list to one kind: `vulnerability`, `malicious`, `unmaintained`,
`unsound`, `notice`, `eol`, `eol_soon`, `platform_deadline`, or
`platform_deadline_soon`. The list and `show` print the kind. A Play
finding links to the help page recorded for that deadline.

`show` prints one finding and its history. `ignore` requires `--reason`.
The reason must not be empty or only whitespace. `--until` is the last UTC
day the ignore stays in force, as `YYYY-MM-DD`. Without `--until` the
ignore does not expire. `reopen` makes an ignored or resolved finding open
again.

`--data-dir` is the same option as on `radar`. `--json` prints the list or
the `show` record as JSON. The fields are described in
[radar.md](radar.md).

Exit status is `0` on success, `1` when the id is unknown or the reason is
empty, and `2` when the command line is wrong.

```console
craftward findings
craftward findings harbor-light --state all --kind vulnerability --json
craftward findings show 1
craftward findings ignore 1 --reason "not affected" --until 2026-12-31
craftward findings reopen 1
```
