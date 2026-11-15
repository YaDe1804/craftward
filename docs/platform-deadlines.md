# Play target-API deadlines

Google Play requires an app's `targetSdk` to meet a level by a published
date. Google does not offer an API for those dates, so Craftward keeps them
in [`data/platform-deadlines.json`](../data/platform-deadlines.json). The
file is compiled into the program. `radar` does not download it.

`craftward deadlines` prints the file in use, its `data_version`, and the
source of each row.

## File

`schema_version` is `1`. `data_version` is the day the file was last
checked, as `YYYY-MM-DD`. `deadlines` is the list of rows.

| Field | Meaning |
| --- | --- |
| `platform` | `google_play`. |
| `form_factor` | `phone`, `automotive`, `wear`, `tv`, or `xr`. |
| `requirement` | A short id. `new_apps_and_updates` covers new apps and app updates when the page gives them the same date. `existing_apps` is the availability requirement for an app that is already published. |
| `api_level` | Minimum `targetSdk` that meets the row. A positive integer. |
| `deadline` | The day the requirement is in force, `YYYY-MM-DD`. That day counts as reached. |
| `source` | `https` URL of the page the row was copied from. |
| `checked_on` | The day this row was compared with that page, `YYYY-MM-DD`. |
| `note` | One short sentence. |

A row is a duplicate when `platform`, `form_factor`, `requirement`, and
`deadline` match another row. The file is rejected. Dates must be real
calendar days. URLs must be `https`. Several rows may share a form factor
and a requirement when their deadlines differ. Radar does not report every
one of them. See [Which row is reported](#which-row-is-reported).

## Which file radar uses

1. The built-in file.
2. `platform-deadlines.json` in the data directory, when that file is
   valid and its `data_version` is strictly newer. The report names it.
3. An older file in the data directory is not used. `deadlines` says so.
4. An invalid file in the data directory is a warning. The built-in file
   is used.

When `data_version` is more than 365 days before today, `radar` and
`deadlines` warn that the deadline data may be out of date. The file is
still used.

## How to update the file

1. Open the Play Console Help page named by `source`. The current page is
   <https://support.google.com/googleplay/android-developer/answer/11926878>.
2. Change a row only when that page states the level and the date. Do not
   add a date Google has not published, including an extension a developer
   may request.
3. Keep Wear OS, Android TV, Android Automotive OS, and Android XR as
   their own rows when the page states different levels.
4. Set `checked_on` on each row you reviewed, and set `data_version` to
   the day of the review.
5. Run `cargo test -p craftward-core --test deadlines`. The test rejects a
   missing field, a bad date, a non-`https` URL, and a duplicate.

A project selects its rows with `stack.android.form_factor`. The default
is `phone`. `store` `none`, or no `stack.android` at all, skips the check.

## Which row is reported

For one form factor and one requirement, `radar` uses at most one row:

1. The newest row whose deadline is today or earlier. When `targetSdk` is
   below that row, the finding is `platform_deadline`. Older rows of the
   same pair are not evaluated.
2. When that row is already met, or there is no such row, the next future
   row. It is `platform_deadline_soon` when its deadline is inside
   `--deadline-warning-days` and `targetSdk` is still below it. A future
   row further out is not used instead.

The finding id is `PLAY:<form factor>:<requirement>`. A later data file
that raises the required level updates that finding. It does not add a
second one. The finding is resolved only when `targetSdk` meets the row
that is current under the rules above. Meeting an older row is not enough
while a newer reached row is still unmet.

## How targetSdk is found

`stack android` records the settings. See [cli.md](cli.md) and
[file-format.md](file-format.md).

1. `target_sdk`, when it is set. The report says `configured`.
2. Otherwise each path in `detect_from`. Craftward reads
   `targetSdk = 35`, `targetSdk 35`, `targetSdkVersion 35`, and
   `targetSdkVersion = 35`. A reference such as
   `libs.versions.targetSdk.get().toInt()` is resolved from the
   `[versions]` table of a `*.toml` file that is also in `detect_from`.
   Only that table is read, and only by Craftward's own reader.
3. Comments (`//`, `/* */`, and `#` in TOML) are ignored.
4. Different values: the highest is used, and the report names the others.
5. No value, or an expression Craftward cannot read: a warning, and no
   finding. Set `target_sdk` for those projects.

`min_sdk` is stored and is not compared with these deadlines.
