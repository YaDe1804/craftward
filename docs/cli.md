# Command-line interface

`craftward` reads and checks project files and keeps a list of them in a
[workspace file](workspace-format.md). With no arguments it prints its name and
version.

Exit status is `0` for success, `1` when a file or request fails, and `2` when
the command line is wrong. `radar` also exits `3` when an actionable
finding is open. Notices and a support end that is only approaching count
with `--strict`. An error still exits `1`. Ignored findings do not change
that status.
`--help` after a command prints that command's help and exits `0`.

`--workspace <FILE>` selects the workspace file for that invocation. Otherwise
Craftward uses `CRAFTWARD_WORKSPACE`, then the default location described in
the workspace format. Every path the program prints uses the separator of
the operating system.

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

Removes one project id from the workspace. The project file stays on disk.

```console
craftward remove harbor-light
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
choice came from the command-line option, `CRAFTWARD_WORKSPACE`, or the
default location. The file does not have to exist yet.

```console
craftward workspace path
craftward --workspace ./workspace.json workspace path
```

## show

Prints one project so its ids can be copied into the commands below. The
header lists id, name, status, and the optional version, description,
source directory, update day, and tags. Each phase lists every task with
its id and status, including tasks that are done or dropped. Open issues
are those with status `open` or `in_progress`. Resolved and `wont_fix`
issues are omitted. `issues` and `watch_items` are always printed; an empty
list says `none`.

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

Changes header fields. At least one option is required. `--version ""` and
`--description ""` remove those fields. `--status` is `idea`,
`in_development`, `released`, `maintenance`, `paused`, or `discontinued`.

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

`phase remove` deletes a phase only when it has no tasks. Otherwise it exits
1 and names `--force`. `--force` removes the phase and its tasks.

An unknown phase id exits 1 and lists the phase ids in the file.

```console
craftward phase add harbor-light --title "Prüfung"
craftward phase remove harbor-light foundation
craftward phase remove harbor-light foundation --force
```

## task

`task add` appends a task to a phase. `--title` is required. The status
defaults to `open`. `--status` is `open`, `in_progress`, `done`, or
`dropped`. `--due` is `YYYY-MM-DD`. Without `--id`, the id is formed the
same way as a phase id and must be unique among that phase's tasks.

`task status` sets the status. `task done` is `task status` with status
`done`. `task remove` deletes one task.

An unknown phase or task exits 1 and lists the ids in that list. A bad
status or date exits 2 and lists the allowed values.

```console
craftward task add harbor-light foundation --title "Write the schema"
craftward task status harbor-light foundation schema in_progress
craftward task done harbor-light foundation schema
craftward task remove harbor-light foundation schema
```

## issue

`issue add` appends an issue with status `open`. `--title` and `--severity`
are required. Severity is `low`, `medium`, `high`, or `critical`. `--url`
must start with `http://` or `https://`. Without `--id`, the id is formed
from the title and must be unique among issues.

`issue status` sets the status to `open`, `in_progress`, `resolved`, or
`wont_fix`. `issue remove` deletes one issue.

An unknown issue exits 1 and lists the issue ids. A bad status, severity, or
URL exits 2 and lists the allowed values.

```console
craftward issue add harbor-light --title Contrast --severity medium
craftward issue status harbor-light theme-contrast in_progress
craftward issue remove harbor-light theme-contrast
```

## watch

`watch add` appends a watch item. `--kind`, `--title`, and `--due` are
required. Kind is `deadline`, `expiry`, `review`, or `other`. `--due` is
`YYYY-MM-DD`. Without `--id`, the id is formed from the title and must be
unique among watch items.

`watch remove` deletes one watch item. An unknown id exits 1 and lists the
watch ids. A bad kind or date exits 2 and lists the allowed values.

```console
craftward watch add harbor-light --kind expiry --title "Renew domain" --due 2027-06-30
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

Exit status is `0` on success, `1` when the name, path, or file is wrong,
and `2` when the command line is wrong. An unknown kind exits 2.

```console
craftward stack add harbor-light --kind runtime --name Node.js --version 22 --eol-product nodejs
craftward stack set harbor-light Node.js --version ""
craftward stack remove harbor-light Node.js
craftward stack lockfile add harbor-light package-lock.json
craftward stack lockfile remove harbor-light package-lock.json
```

## radar

Looks up known vulnerabilities for the packages in the lockfiles named by
`stack.lockfiles`. With no project, every project in the workspace is
checked. A project argument is an id or a path, with the same ambiguity
rule as `roadmap`.

`--refresh` ignores cached answers and asks again. `--offline` never
contacts OSV or endoflife.date. The two options cannot be combined.
`--strict` makes notices and `eol_soon` count toward exit status 3.
`--eol-warning-days` is how many days ahead a support end still counts as
`eol_soon`. The default is 90. `--cache-dir` selects the
cache directory; otherwise `CRAFTWARD_CACHE_DIR` is used, then the default
described in [radar.md](radar.md). `--data-dir` selects the database
directory; otherwise `CRAFTWARD_DATA_DIR` is used, then the default
described in [storage.md](storage.md). `--json` prints the same report as
JSON. `--all` includes ignored findings in the list. They are always counted.

Exit status is `0` when no actionable finding is open, `3` when a
vulnerability, a malicious package, or a past support end is open, `1`
when a project could not be checked, and `2` when the command line is
wrong. Notices and `eol_soon` count toward status 3 only with `--strict`.
An error takes priority over an open finding. The text
report, the JSON fields, the lifecycle, and what is sent to OSV are
described in [radar.md](radar.md).

```console
craftward radar
craftward radar harbor-light --json
craftward radar --offline --cache-dir ./cache --data-dir ./data
craftward radar --strict --eol-warning-days 30
```

## findings

Reads the findings stored by `radar`. It does not contact OSV. With no
project, every stored finding is listed. A project argument is an id or a
path, with the same ambiguity rule as `roadmap`. `--state` is `open`,
`ignored`, `resolved`, or `all`. The default is `open`. `--kind` limits
the list to one kind: `vulnerability`, `malicious`, `unmaintained`,
`unsound`, `notice`, `eol`, or `eol_soon`. The list and `show` print the
kind.

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
