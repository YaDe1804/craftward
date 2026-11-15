# ADR 0013: Roadmap generation

- Status: Accepted
- Date: 2026-11-15

## Decision

`craftward roadmap` writes a standalone HTML file named
`<id>_roadmap_vN.html`. `N` is one higher than the highest existing number in
the output directory, starting at 1. An existing file is never overwritten.
The finished text is hard-linked into place. When the operating system
reports that the file system does not support hard links, the file is
created exclusively instead, and removed if that write fails.

The page embeds its CSS. It does not load external resources and it does not
run JavaScript. Project text is escaped before it is inserted. Links use the
already validated `http` and `https` addresses and
`rel="noopener noreferrer"`.

Fixed labels live in one table of keys, with English and German. English is
the default. Project content is not translated. The table is the first step
of the multilingual interface.

## Rationale

English is the default language. All UI strings go through translation keys
from the first line of UI code. English and German are available at launch.
A versioned file that is never replaced keeps an older roadmap next to the
new one. Embedded CSS and escaped project text keep the page self-contained.
