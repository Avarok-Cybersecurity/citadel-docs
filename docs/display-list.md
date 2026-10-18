# Display list format, version 5

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
| 4      | `u16`  | version, `5`                                              |
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
| `uv`      | char table length `C` |
| `C` × (`uv`, `uv`) | char table: `(gap, glyph index)`, in increasing code-point order; the entry's code point is the previous entry's plus one plus `gap` (the first entry's is `gap`). Each code point is a Unicode scalar value and each glyph index < `G` |

The glyph table lists each glyph id the list's runs draw in that face, once, in order of first
use, with its nominal advance: what a painter preloads into a glyph atlas before it reaches the
bodies. A run's glyphs are named by their **glyph index**, their position in this table (they
are the shaper's glyphs, so a ligature or an alternate the `cmap` never names appears here
too); a run cannot draw a glyph its face does not list. Every face of the layout is present,
even one a partial list does not use (its tables are then empty), so face ids mean the same in
full and partial lists.

The **char table** says which glyph a character is predicted to be: the glyph most of the
list's clusters starting with that character begin with. It is a prediction, not a `cmap`: a
glyph that differs is listed in its run (see Placing a `GLYPH_RUN`). A character with no entry
predicts glyph index 0.

### Strike table

After the last face, still inside the face table section (which ends exactly at the string
table): `uv` strike count `S`, then per strike (its position is its **strike id**, as runs
refer to it):

| Type | Field |
|------|-------|
| `uv` | face id (< face count) |
| `zz` | font size, Pt100 |
| `G` × `zz` | the **base advance** of each glyph of that face's glyph table, Pt100, in glyph-index order (`G` is that face's glyph table length) |

The writer sets each base to `floor(advance · size / units_per_em)`, the glyph's nominal
advance rounded down; a shaped advance is that or one more unless kerning or justification
moved it. A reader never computes it: it adds to the stored base. Each face and size a run
uses is one strike, in order of first use.

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
| `0x11` | `GLYPH_RUN` | run head, `str` text, `uv` glyph count `n`, `u8` column flags, the columns the flags name (see Placing a `GLYPH_RUN`) |
| `0x20` | `IMAGE` | `rect`, `uv` string index of the content id |
| `0x21` | `LINK` | `rect`, `uv` string index of the target |
| `0x22` | `SIG_FIELD` | `rect`, `uv` string index of the field id, `uv` string index of the signer role: an unsigned signature field (the PDF writes a `/Sig` form field; a painter may draw nothing) |
| `0x30` | `CLIP_PUSH` | `rect`: intersect the clip with it until the matching pop |
| `0x31` | `CLIP_POP` | empty: restore the clip before the matching push |
| `0x40` | `CARET_LINES` | a page's lines of document text: where each came from, its caret extent and how many runs it is (see below); painted by nobody |

**Path segment**: a `u8` verb then its points as `zz` pairs: `0` move (1 point), `1` line
(1 point), `2` cubic Bézier (control 1, control 2, end: 3 points), `3` close (none).

**Run head**: `uv` strike id (its face and size), `rgba` fill, `zz` x, `zz` y. `(x, y)` is the
left end of the baseline.

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

#### The glyphs: predictions and columns

Glyph `i` (0-based) has a **prediction**, computed from the glyph before it:

- **cluster**: 0 for glyph 0; otherwise the previous glyph's cluster plus the UTF-8 length of
  the character starting there (plus 0 when the previous cluster is the end of `text`);
- **glyph index**: the char table entry of the character starting at the glyph's cluster (its
  actual cluster, after the cluster column), or 0 when there is none or the cluster is the end
  of `text`;
- **advance**: the strike's base advance of the glyph's (actual) index, plus 1 when its bit
  is set in the plus-one bitmap;
- **x offset** and **y offset**: 0.

The column flags byte says which columns follow, in this order; a column is present only when
it lists at least one glyph, and any other bit is malformed:

| Bit | Column | Present as | Entry value |
|----:|--------|------------|-------------|
| `0x01` | glyph indices | sparse | `uv` glyph index (< `G`) |
| `0x02` | clusters | sparse | `zz` actual cluster minus predicted |
| `0x04` | plus-one bitmap | `ceil(n / 8)` bytes | — bit `i` (byte `i >> 3`, mask `1 << (i & 7)`) adds 1 to glyph `i`'s base advance; padding bits are 0 |
| `0x08` | advances | sparse | `zz` actual advance minus the base (32-bit wrapping; the bit is then ignored) |
| `0x10` | x offsets | sparse | `zz` x offset |
| `0x20` | y offsets | sparse | `zz` y offset |

A **sparse** column is `uv` byte length `L`, then `L` bytes of entries, each `uv` gap then the
value; the entry is for glyph `previous + 1 + gap` (the first entry's glyph is `gap`), so
entries are in increasing glyph order. A glyph a column lists takes the entry's value; every
other glyph takes its prediction. Every entry is for a glyph `< n`, and the entries fill `L`
exactly.

A run of Latin text set without ligatures or kerning, the common case, is its head, its text,
`n`, the flags and its bitmap: about one byte a glyph, where version 2 spent seven.

Decoding, in order, for `i` in `0..n`:

```
cluster  = predicted cluster;  if clusters lists i: cluster += value
ch, w    = character at cluster in text (w = its UTF-8 length; none, w = 0, at the end)
index    = glyph indices lists i ? value : (char table[ch] or 0)
id       = glyph table[index].id
advance  = advances lists i ? base[index] + value : base[index] + bit(i)      (wrapping i32)
x, y     = offset columns' value or 0
next predicted cluster = cluster + w
```

A TypeScript reader keeps one cursor (byte position, pending glyph, pending value) per present
column and the bitmap's offset, all into the payload, so nothing is copied.

Two runs from `every-op.bin` (face 1, `units_per_em` 2048, glyph table `65535/0, 3/1130,
300/1200, 301/0`; strike 0 is face 1 at size 1100, bases `0, 606, 644, 0`; char table
`f→0, ' '→1, é→2`):

```
11 10                       GLYPH_RUN, 16 bytes
00 | ff 00 00 80 | c0 70 | 01   strike 0, colour, x 7200, y −1
04 66 c3 a9 20              text "fé "
03 | 04                     3 glyphs; flags: plus-one bitmap
02                          bitmap: glyph 1 is base + 1
→ (65535, cluster 0, 0), (300, 1, 645), (3, 3, 606), no offsets

11 2d                       GLYPH_RUN, 45 bytes
00 | ff 00 00 80 | c0 70 | 01 | 05 66 69 20 c3 a9 | 04 | 3b
02  03 03                   glyph indices: glyph 3 is index 3 (301)
04  01 02  01 03            clusters: glyph 1 +1 (2), glyph 3 −2 (3)
08  00 80 08  00 c9 09  00 57 05     advances: +512, −613, −44 from the base
05  00 27  02 d7 04         x offsets: glyph 0 −20, glyph 3 −300
05  00 50  02 8f 03         y offsets: glyph 0 +40, glyph 3 −200
→ (65535, 0, 512, −20, 40), (3, 2, −7), (300, 3, 600), (301, 3, 0, −300, −200)
```

`TEXT_RUN` (`0x10`, unshaped text placed from `cmap` glyphs and nominal advances) was version
1's and is retired: version 2 and later have no unshaped text, and `0x10` is not a valid opcode.
Version 3 added the document table and `SIG_FIELD`, which the PDF writer needs to draw from
the list alone. Version 5 replaced version 2's five varints a
glyph with predictions and sparse columns (the char and strike tables, the strike id in the
run head, the column flags): the same glyphs, clusters, advances and offsets in a quarter of
the bytes, decoded in less time.

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

- magic, version `5` (any other, earlier ones included, is refused by number), flags `0`,
  sections in order, tables exactly filled (the face table section ends exactly after the
  strike table; the document table ends exactly at the page index);
- every face id < face count; `units_per_em ≠ 0`; every glyph id ≤ 65535; every char table
  entry a Unicode scalar value with a glyph index < `G`; every strike's face id < face count;
- a `GLYPH_RUN`'s strike id < strike count; its text is UTF-8; its column flags known; each
  sparse column's entries for glyphs `< n` and exactly filling its length; its bitmap's padding
  clear; every listed glyph index < `G`; a run with glyphs is not in a face with an empty glyph
  table; every listed cluster is a character boundary of the text, at most its length, and not
  before the previous glyph's cluster (so clusters never decrease);
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
