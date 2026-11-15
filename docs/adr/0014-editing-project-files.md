# ADR 0014: Editing project files

- Status: Accepted
- Date: 2026-11-15

## Decision

Changes to `craftward.json` are made by functions in `craftward-core`. The
command-line tool parses arguments and prints the result. A change is written
only when the project is valid afterwards. Otherwise the file is left
unchanged and every error is reported.

The replacement is atomic. The stored text uses two-space indentation, LF
line endings, a trailing newline, the field order of the Rust types, and
non-ASCII characters as themselves. The first write may reformat a file that
was edited by hand.

When an add command omits `--id`, the id is formed from the title. `ä ö ü ß`
become `ae oe ue ss`. Other accented Latin letters become their base letter
when that fold is known, and other letters are dropped. Characters outside
`a-z` and `0-9` become hyphens, which are collapsed and trimmed. The result
is at most 64 characters and does not end with a hyphen. An id that is
already used in that list gets `-2`, `-3`, and so on. A title that yields no
id is rejected.

Every successful change sets `updated` to the UTC day from the same source
used when a roadmap is generated.

## Rationale

The desktop application will call the same functions as the command line. A
file is replaced only when validation accepts it, so an edit cannot store a
project the tools would reject. The id and the `updated` day are chosen in
that same step, so every caller stores them the same way. One stored form
means a later edit does not depend on how the file was first written.
