# Command-line interface

`craftward` reads and checks project files and keeps a list of them in a
[workspace file](workspace-format.md). With no arguments it prints its name and
version.

Exit status is `0` for success, `1` when a file or request fails, and `2` when
the command line is wrong. `--help` after a command prints that command's
help and exits `0`.

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
its location, for example `plan.phases[0].tasks[1].due`.

```console
craftward validate
craftward validate examples/minimal.craftward.json
```

## workspace path

Prints the workspace file that this invocation would use, and whether that
choice came from the command-line option, `CRAFTWARD_WORKSPACE`, or the
default location. The file does not have to exist yet.

```console
craftward workspace path
craftward --workspace ./workspace.json workspace path
```
