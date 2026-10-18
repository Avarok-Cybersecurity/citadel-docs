# Display list format, version 4

The display list is the one description of every page that every painter draws from: the
canvas editor (TypeScript, Canvas2D), the SVG preview (`cdoc_paint::svg`) and the PDF writer
(`cdoc_pdf::render_pdf`). It is produced by `cdoc-paint` (`encode`, `encode_pages`) and exported to the browser by
`cdoc-wasm` (`pm_to_display_list`, `pm_to_display_list_pages`) as a `Uint8Array`. This document is the
contract a reader implements; `crates/cdoc-paint/src/format.rs` holds the constants and the
golden files in `crates/cdoc-paint/tests/data/` pin the bytes.

## Conventions

- **Byte order**: little-endian for every fixed-width integer.
- **`u8`, `u16`, `u32`, `i32`**: fixed-width integers.
- **`uv`**: an unsigned LEB128 varint of at most 5 bytes holding a value that fits 32 bits:
  seven bits per byte, least significant group first, high bit set on every byte but the last.
  A 5th byte above `0x0F`, or a 6th byte, is malformed.
- **`zz`**: a signed 32-bit value, zigzag-folded to unsigned (`(v << 1) ^ (v >> 31)`; 0, −1, 1,
  −2 → 0, 1, 2, 3) and written as a `uv`. Decode: `(u >>> 1) ^ -(u & 1)`.
- **`str`**: a `uv` byte length followed by that many bytes of UTF-8.
- **`rgba`**: four `u8`s, red, green, blue, alpha; alpha is straight (not premultiplied),
  255 opaque.
- **Units**: every coordinate, length and font size is **Pt100**, hundredths of a PostScript
  point (7200 = one inch), as an `i32`. **Page space**: origin at the sheet's top-left corner,
  x right, y **down**. No floating-point value appears anywhere in the format.
- **`rect`**: four `zz`: `x`, `y` (top-left corner), `width`, `height`.

## Layout of the buffer

```
+--------+------------+--------------+----------------+------------+-----------------------------+
| header | face table | string table | document table | page index | page bodies, in index order |
+--------+------------+--------------+----------------+------------+-----------------------------+
0        24
```

Every section precedes every page body, so a reader holding a prefix of the buffer that ends
after page N's body can open pages 0..=N.

### Header (24 bytes)

| Offset | Type   | Field                                                     |
|-------:|--------|-----------------------------------------------------------|
| 0      | 4×`u8` | magic `CDPL` (`43 44 50 4C`)                              |
| 4      | `u16`  | version, `4`                                              |
| 6      | `u16`  | flags, `0` (reserved; a reader refuses any other value)   |
| 8      | `u32`  | page count `P`                                            |
| 12     | `u32`  | face table offset, always `24`                            |
| 16     | `u32`  | string table offset                                       |
| 20     | `u32`  | page index offset                                         |

Invariant: `24 = faces ≤ strings ≤ index`, and each table ends exactly where the next section
begins (no padding, no trailing bytes).

### Face table

`uv` face count, then per face (its position is its **face id**, as runs refer to it):

| Type      | Field |
|-----------|-------|
| `u8`      | style: bit 0 bold, bit 1 italic; other bits 0 |
| `str`     | family, exactly as the font port served it (the same name the UI registers with `@font-face`) |
| `uv`      | `units_per_em`, 1..=65535 |
| `uv`      | glyph table length `G` |
| `G` × (`uv`, `uv`) | glyph table: `(glyph id, advance)`, both ≤ 65535; the advance is the face's `hmtx` advance in font units |

The glyph table lists each glyph id the list's runs draw in that face, once, in order of first
use, with its nominal advance: what a painter preloads into a glyph atlas before it reaches the
bodies. Runs carry glyph ids themselves (they are the shaper's, so a ligature or an alternate
the `cmap` never names appears here too). Every face of the layout is present, even one a
partial list does not use (its table is then empty), so face ids mean the same in full and
partial lists.

### String table

`uv` string count, then each string as a `str`. Image content ids, link targets, signature
field ids and roles, and the document table's title, language and headings refer to strings
by index. Each distinct string is stored once.

### Document table

It begins where the string table's last string ends and ends exactly at the page index (the
header has no offset for it). It is the whole document's, in a partial list too.

| Type | Field |
|------|-------|
| `uv` | title: `0` none, else string index + 1 |
| `uv` | language (BCP 47): `0` none, else string index + 1 |
| `uv` | heading count `H` |
| `H` × (`u8`, `uv`, `zz`, `uv`) | outline, in document order: level (0 outermost), page number in the layout, `y` of the top of the heading's first line (Pt100, page space), string index of the heading text |

A heading's page number may name a page a partial list does not hold.

### Page index (`P` × 20 bytes)

| Offset | Type  | Field |
|-------:|-------|-------|
| 0      | `u32` | page number in the layout (zero-based); equals the slot in a full list |
| 4      | `u32` | body offset from the start of the buffer |
| 8      | `u32` | body length in bytes |
| 12     | `i32` | page width, Pt100 |
| 16     | `i32` | page height, Pt100 |

Entry `k` is at `index + 20·k`, so any page is reached without reading the others. A body
lies entirely after the index. `encode_pages` writes only the pages asked for, in the order
asked, each keeping its page number: replace those pages in the painter's cache.

### Page body

A sequence of operations in painting order (later paints over earlier) until the body's length
is used up. Each operation is:

| Type | Field |
|------|-------|
| `u8` | opcode |
| `uv` | payload length `L` |
| `L` bytes | payload |

The payload of each opcode is consumed exactly; a payload longer or shorter than its fields
is malformed. The length prefix lets a reader skip an operation it does not paint without
parsing it.

## Operations

| Code | Name | Payload |
|-----:|------|---------|
| `0x01` | `FILL_RECT` | `rect`, `rgba` |
| `0x02` | `STROKE_RECT` | `rect`, `zz` width, `rgba`; the stroke is centred on the rectangle's edges, mitre joins |
| `0x03` | `LINE` | `zz` x1, `zz` y1, `zz` x2, `zz` y2, `zz` width, `rgba`; butt caps. Rules, table borders, underlines and strikethrough arrive as `LINE` (decorations are horizontal lines the layout already placed) |
| `0x04` | `FILL_PATH` | `rgba`, `uv` segment count, segments; nonzero winding |
| `0x05` | `STROKE_PATH` | `zz` width, `rgba`, `uv` segment count, segments; round caps and joins |
| `0x11` | `GLYPH_RUN` | run head, `str` text, `uv` glyph count `n`, `n` × (`uv` glyph id, `uv` cluster, `zz` advance, `zz` x offset, `zz` y offset) |
| `0x20` | `IMAGE` | `rect`, `uv` string index of the content id |
| `0x21` | `LINK` | `rect`, `uv` string index of the target |
| `0x22` | `SIG_FIELD` | `rect`, `uv` string index of the field id, `uv` string index of the signer role: an unsigned signature field (the PDF writes a `/Sig` form field; a painter may draw nothing) |
| `0x30` | `CLIP_PUSH` | `rect`: intersect the clip with it until the matching pop |
| `0x31` | `CLIP_POP` | empty: restore the clip before the matching push |
| `0x40` | `CARET_LINES` | a page's lines of document text: where each came from, its caret extent and how many runs it is (see below); painted by nobody |

**Path segment**: a `u8` verb then its points as `zz` pairs: `0` move (1 point), `1` line
(1 point), `2` cubic Bézier (control 1, control 2, end: 3 points), `3` close (none).

**Run head**: `uv` face id, `zz` font size, `rgba` fill, `zz` x, `zz` y. `(x, y)` is the left
end of the baseline.

### Placing a `GLYPH_RUN` (shaped text)

Every run is shaped: the layout set it through `cdoc-shape` with the run's features (standard
and contextual ligatures, kerning, mark positioning; Word's defaults when the document says
nothing), so the glyph ids, advances and offsets are final and a painter never consults a
`cmap`. The pen starts at `x`. Glyph `i` is drawn at `(pen + x_offset_i, y + y_offset_i)`
(offsets in page space, y down), then the pen moves by `advance_i`. Offsets do not move the
pen. The advances are the layout's own (font units scaled cumulatively along each word, then
differenced), so the pen ends exactly at the run's measured width, and a justified line's
widened spaces arrive as wider space advances.

`text` is the characters the glyphs stand for, which selection, search and the PDF's
`ToUnicode` read. Glyph `i`'s **cluster** is the byte offset in `text` of the first character
it stands for; clusters never decrease along a run, and glyphs of one cluster share it. A
cluster's text runs from its offset to the next larger cluster (or the end of `text`): a
ligature is one glyph whose cluster covers several characters, a decomposed accent several
glyphs over one. A painter that places characters for selection spreads a cluster's
characters evenly over the advances of its glyphs.

`TEXT_RUN` (`0x10`, unshaped text placed from `cmap` glyphs and nominal advances) was version
1's and is retired: version 2 and later have no unshaped text, and `0x10` is not a valid opcode.
Version 3 added the document table and `SIG_FIELD`, which the PDF writer needs to draw from
the list alone.

### `CARET_LINES` (where the text came from)

A page with lines of document text carries one `CARET_LINES` op, after its links and signature
fields, holding every line the layout set on it in the layout's order: the body's lines in flow
order (each column's footnotes after it), then the header's and the footer's. It is not
painted (a painter skips the whole op at once); an editor maps carets, selections and clicks
through it without reading the drawn text. Payload: `uv` line count (at least 1), then per
line:

| Type | Field |
|------|-------|
| `u8` | head: flags `0x01` new paragraph (its provenance follows; else the previous line's), `0x02` same pitch (its top is as far below the previous line's as that one's was below its own; else the step follows), `0x04` same height (else it follows), `0x08` no stops (else they follow); other bits `0`. The first line names its paragraph; before it, the top, step and height are `0` |
| `u8` | new paragraph only: region, `0` body, `1` header, `2` footer, `3` footnote |
| `uv` | new paragraph only: section index (body, header, footer) or footnote id (footnote) |
| `u8` | new header or footer paragraph only: which one, `0` default, `1` first page, `2` even pages |
| `uv` + `uv`s | new paragraph only: path, count then block indices from the region's block list down to the paragraph (`[block]`, or `[table, row, cell, block]` inside a table, cells counted within their row, and so on for nested tables) |
| `zz` | unless same pitch: the step from the previous line's caret top (page space) |
| `zz` | unless same height: caret bottom less caret top |
| `uv` | runs: how many of the page's runs of document text, after the previous lines', are this line's |
| `uv` + stops | unless no stops: stop count (at least 1), then per stop `uv` gap and `zz` x: the stops no run draws (a tab, an image, a form or signature field, a page field), and an empty line's one stop |

A **gap** is a position between the paragraph's units: its characters and inline objects (tab,
break, image, field, footnote reference, form field, signature field), one unit each, counted
as the document holds them, hidden ones (a deletion the revision view leaves out) included.
Gap 0 is the paragraph's start, gap `k` follows its `k`-th unit. Gaps strictly increase along a
line. A list label, a footnote's own number and the hyphen a broken word ends in are no units,
so they have no stops; an empty line has the one stop of the gap it stands at. A table's
header rows repeated on a continuation page carry no caret lines: the original rows do.

The stops of a line's text are not in the list: computing them costs every layout, exports
included, what only an editor needs. The layout keeps, per glyph run, which unit each
character draws, and `cdoc_layout::caret_stops` puts a page's stops together on demand: per
line, its runs' stops (from their clusters and advances, a cluster's width shared by its
characters, a zero-width character after another sharing its stop) merged with the line's own
stops in x order. The editor calls it for the pages it derives, so a reader of the list alone
has each line's provenance, extent and object stops, not the stop of every character.

Version 4 added `CARET_LINES`; nothing else changed from version 3. A version 3 reader refuses a
version 4 list by its version field, as every reader refuses a version it was not written for.

### Drawing a glyph

A glyph is its outline (see Glyph paths) scaled by `size / 100 / units_per_em` points per font
unit, with the y axis flipped (outlines are y up, the page is y down), its origin at the pen on
the baseline, filled with the run's colour, nonzero winding.

### Links

`LINK` targets are stored exactly as the document holds them. A painter honours only
`http://`, `https://` and `mailto:` targets (case-insensitive, leading whitespace ignored);
`cdoc_paint::allowed_link` is the rule.

### Invariants a reader checks (and `cdoc_paint::decode` enforces)

- magic, version `4`, flags `0`, sections in order, tables exactly filled (the document table
  ends exactly at the page index);
- every face id < face count; `units_per_em ≠ 0`; every glyph id ≤ 65535;
- a `GLYPH_RUN`'s text is UTF-8 and its clusters never decrease and each falls on a
  character boundary of the text;
- every string index < string count; every string is UTF-8;
- every opcode is in the table above and its payload is consumed exactly;
- a `CARET_LINES` op has at least one line, its first line names its paragraph, its heads,
  region and kind codes are those above, a line with no runs has at least one stop, and its
  gaps strictly increase;
- clip pushes and pops balance within each page: never a pop without a push, none open at the
  end of the body;
- a body lies after the page index and inside the buffer.

`decode` checks the header and tables once and each body the first time the page is opened;
iterating an opened page cannot fail.

## Glyph paths

`glyph_path(assets, family, bold, italic, glyph_id)` (`cdoc_paint::glyph::glyph_path`) returns
one glyph's outline for the face a face-table entry names, for a glyph atlas:

| Offset | Type | Field |
|-------:|------|-------|
| 0 | `u8` | path format version, `1` |
| 1 | `u8` | reserved, `0` |
| 2 | `u16` | command count `C` |
| 4 | `i16` × 4 | bounding box `x_min`, `y_min`, `x_max`, `y_max` (all 0 for a glyph with no outline) |
| 12 | … | `C` commands |

Each command is a `u8` verb followed by `i16` coordinates in **font units, y up**, rounded:

| Verb | Command | Coordinates |
|-----:|---------|-------------|
| 0 | move to | x y |
| 1 | line to | x y |
| 2 | quadratic to | cx cy x y |
| 3 | cubic to | c1x c1y c2x c2y x y |
| 4 | close | — |

A glyph with no outline (a space) has `C = 0`. A glyph id the face does not have is an error.

## Face metrics

`face_metrics(assets, family, bold, italic)` (`cdoc_paint::glyph::face_metrics`) returns 8 bytes:
`u16 units_per_em`, `i16 ascender`, `i16 descender` (negative), `i16 line_gap`, in font units,
from `hhea` (or `OS/2` typographic metrics when the font sets `USE_TYPO_METRICS`).

## Versioning

Any change to a layout described here is a new version: bump `format::VERSION`, update this
document, regenerate the golden files (`cargo run -p cdoc-paint --example emit_golden --
crates/cdoc-paint/tests/data`) and update their checksums in `tests/golden.rs`. A reader
refuses any version it was not written for.
