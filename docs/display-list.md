# Display list format, version 1

The display list is the one description of every page that every painter draws from: the
canvas editor (TypeScript, Canvas2D), the SVG preview (`cdoc_paint::svg`) and, next, the PDF
writer. It is produced by `cdoc-paint` (`encode`, `encode_pages`) and exported to the browser by
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
+--------+------------+--------------+------------+--------------------------+
| header | face table | string table | page index | page bodies, in index order |
+--------+------------+--------------+------------+--------------------------+
0        24
```

Every section precedes every page body, so a reader holding a prefix of the buffer that ends
after page N's body can open pages 0..=N.

### Header (24 bytes)

| Offset | Type   | Field                                                     |
|-------:|--------|-----------------------------------------------------------|
| 0      | 4×`u8` | magic `CDPL` (`43 44 50 4C`)                              |
| 4      | `u16`  | version, `1`                                              |
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

The glyph table lists each glyph the list's text runs set in that face, once, in order of first
use. Text runs refer to glyphs by their **index in this table**, so the common glyphs of a face
take one byte each. Every face of the layout is present, even one a partial list does not use
(its table is then empty), so face ids mean the same in full and partial lists.

### String table

`uv` string count, then each string as a `str`. Image content ids and link targets refer to
strings by index. Each distinct string is stored once.

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
| `0x10` | `TEXT_RUN` | run head, `uv` glyph count `n`, `n` × `uv` glyph-table index |
| `0x11` | `GLYPH_RUN` | run head, `uv` glyph count `n`, `n` × (`uv` glyph id, `zz` advance, `zz` x offset, `zz` y offset) |
| `0x20` | `IMAGE` | `rect`, `uv` string index of the content id |
| `0x21` | `LINK` | `rect`, `uv` string index of the target |
| `0x30` | `CLIP_PUSH` | `rect`: intersect the clip with it until the matching pop |
| `0x31` | `CLIP_POP` | empty: restore the clip before the matching push |

**Path segment**: a `u8` verb then its points as `zz` pairs: `0` move (1 point), `1` line
(1 point), `2` cubic Bézier (control 1, control 2, end: 3 points), `3` close (none).

**Run head**: `uv` face id, `zz` font size, `rgba` fill, `zz` x, `zz` y. `(x, y)` is the left
end of the baseline.

### Placing a `TEXT_RUN` (unshaped text)

Glyph ids came from the face's `cmap` (`.notdef`, 0, where it has none: exactly what the PDF
draws). With `units` the running sum of the table advances of glyphs 0..=i, glyph `i + 1`
starts at

```
pen(units) = floor((units · size + floor(units_per_em / 2)) / units_per_em)    (Pt100)
x_i+1 = x + pen(units_0..=i),   x_0 = x
```

The layout measures text with the same rule, so a run's pen ends exactly at its measured width
and no glyph is more than half a hundredth of a point from where the PDF's `Tj` places it.
Kerning is off: an unshaped run's glyphs advance by their nominal widths only. In JavaScript the
products stay below 2^53 for any real run, so `Math.floor` on doubles is exact.

### Placing a `GLYPH_RUN` (shaped text)

The pen starts at `x`. Glyph `i` is drawn at `(pen + x_offset_i, y + y_offset_i)` (offsets in
page space, y down), then the pen moves by `advance_i`. Offsets do not move the pen.

### Drawing a glyph

A glyph is its outline (see Glyph paths) scaled by `size / 100 / units_per_em` points per font
unit, with the y axis flipped (outlines are y up, the page is y down), its origin at the pen on
the baseline, filled with the run's colour, nonzero winding.

### Links

`LINK` targets are stored exactly as the document holds them. A painter honours only
`http://`, `https://` and `mailto:` targets (case-insensitive, leading whitespace ignored);
`cdoc_paint::allowed_link` is the rule.

### Invariants a reader checks (and `cdoc_paint::decode` enforces)

- magic, version `1`, flags `0`, sections in order, tables exactly filled;
- every face id < face count; every glyph-table index < that face's table length;
  `units_per_em ≠ 0`; every glyph id ≤ 65535;
- every string index < string count; every string is UTF-8;
- every opcode is in the table above and its payload is consumed exactly;
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
