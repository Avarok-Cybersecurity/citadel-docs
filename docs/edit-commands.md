# Editor records, version 2

The browser drives the editor (`cdoc-edit`, exported by `cdoc-wasm` as `EditorHandle`) with
binary records: a **request** in, a **result** out. Remote updates, awareness and hit tests
use the records below too. Nothing on this path is JSON and nothing is a float.
`crates/cdoc-edit/src/wire/` is the implementation; `crates/cdoc-edit/tests/wire.rs` round-trips
every request and refuses every truncation.

## Conventions

- **`u8`**: one byte.
- **`uv`**: an unsigned LEB128 varint of up to 64 bits: seven bits per byte, least significant
  group first, high bit set on every byte but the last. A tenth byte above `0x01` is malformed.
- **`zz`**: a signed value zigzag-folded (`(v << 1) ^ (v >> 63)`) and written as a `uv`.
- **`str`**: a `uv` byte length, then UTF-8. **`bytes`**: a `uv` length, then the bytes.
- **`bool`**: a `u8`, 0 or 1.
- **`pt`**: a `zz` holding a Pt100 value (hundredths of a point, 7200 = one inch) that fits
  an `i32`. **`rect`**: four `pt`: `x`, `y` (top-left corner, page space, y down), `width`,
  `height`.
- **`position`**: `client:uv clock:uv`, the id of the unit just before the gap. Positions are
  stable CRDT anchors: they survive local and remote edits. **JavaScript treats them as
  opaque bytes**: copy them out of a result or a hit test and back into a request. Both
  numbers are 64-bit; read them as `BigInt` if they must be read at all.
- **`value`**: a `bytes` holding a model value in the `cdoc_crdt::codec` encoding, the JSON
  data model in bytes (tag `u8` then body: 0 null, 1 false, 2 true, 3 unsigned `uv`,
  4 negative `-1 - n` as `uv n`, 5 string, 6 byte string, 7 sequence `uv count` values,
  8 map `uv count` key/value pairs). The value has the shape the model's JSON gives it
  (`RunProperties`, `ParagraphProperties`, `PageSetupPatch`, `Comment`); absent optional fields
  are omitted. Lengths in values are Pt100 integers.
- Every record is read to its end: trailing bytes are an error, as is a truncated record.
- **Client ids** are `u64`, a `BigInt` in JavaScript (`EditorHandle.blank(1n, fonts)`).

## Requests

A request is one `u8` opcode followed by its fields.

| op | request | fields |
|---|---|---|
| `0x01` | InsertText | `text:str` (`\n` splits the paragraph, `\t` is a tab) |
| `0x02` | DeleteBackward | `unit:u8` (0 char/cluster, 1 word) |
| `0x03` | DeleteForward | `unit:u8` |
| `0x04` | SplitParagraph | |
| `0x05` | InsertBreak | `kind:u8` (0 line, 1 page, 2 column) |
| `0x06` | InsertTab | |
| `0x07` | ToggleMark | `mark:u8` (0 bold, 1 italic, 2 underline, 3 strike, 4 subscript, 5 superscript) |
| `0x08` | SetRunProperty | `patch:value` (`RunProperties`; set fields apply, absent ones are kept) |
| `0x09` | SetParagraphProperty | `patch:value` (`ParagraphProperties`) |
| `0x0A` | SetParagraphStyle | `style:str` |
| `0x0B` | SetNumbering | `num_id:uv level:u8` |
| `0x0C` | ClearNumbering | |
| `0x0D` | Indent | |
| `0x0E` | Outdent | |
| `0x0F` | SetSectionProperty | `patch:value` (`PageSetupPatch`, below; version 2) |
| `0x10` | InsertTable | `rows:uv cols:uv` |
| `0x11` | InsertRowAbove | |
| `0x12` | InsertRowBelow | |
| `0x13` | InsertColumnLeft | |
| `0x14` | InsertColumnRight | |
| `0x15` | DeleteRow | |
| `0x16` | DeleteColumn | |
| `0x17` | DeleteTable | |
| `0x18` | MergeCells | |
| `0x20` | InsertImage | `content_id:str width:pt height:pt` |
| `0x21` | InsertFootnote | `id:uv` |
| `0x22` | InsertLink | `href:str` (links the selection, or inserts the URL as linked text) |
| `0x23` | RemoveLink | |
| `0x24` | InsertComment | `comment:value` (`Comment`) |
| `0x30` | Replace | `from:position to:position text:str` |
| `0x31` | SelectAll | |
| `0x32` | SetSelection | `anchor:position head:position` |
| `0x40` | Undo | |
| `0x41` | Redo | |
| `0x42` | BreakUndoGroup | (call on blur, after a pause, before a paste) |
| `0x43` | MoveCaret | `granularity:u8 forward:bool extend:bool` (granularity 0 char, 1 word, 2 line boundary, 3 line, 4 paragraph, 5 document; `extend` is Shift) |
| `0x44` | SelectWord | `at:position` (double click) |
| `0x45` | SelectParagraph | `at:position` (triple click) |
| `0x50` | CompositionStart | |
| `0x51` | CompositionUpdate | `text:str caret:uv` (characters into `text`) |
| `0x52` | CompositionCommit | |
| `0x53` | CompositionCancel | |

Opcodes `0x01`–`0x32` are document commands: each runs as **one** CRDT transaction and one
undo step (consecutive typing joins one step per word; a caret move or `0x42` ends it). A
command that does not apply where the selection is (merging cells outside a table, removing
a link where there is none) fails with an error and changes nothing; one with nothing to do
(Backspace at the start of the document) returns an empty update. Composition text is a
local overlay, not in the document, until `0x52` inserts it as typing; the UI draws the
provisional text at the result's caret.

## Result

Every `request` and `applyRemote` returns:

```text
result := "CDE" 0x02
          update:bytes                    the update to relay; empty when nothing changed
                                          (always empty for applyRemote)
          count:uv page:uv{count}         pages to repaint, ascending
          page_count:uv
          anchor:position head:position   the selection
          caret:(0 | 1 page:uv rect)      the caret (zero width, line height)
          count:uv (page:uv rect){count}  the selection highlight, one rect per line
          formatting                      (new in version 2)

formatting := run:value                   RunProperties after the cascade
              run_direct:value            RunProperties set directly on the run
              paragraph:value             ParagraphProperties after the cascade
              paragraph_direct:value      ParagraphProperties set directly on the paragraph
              run_mixed:uv                bit set, run keys (below)
              paragraph_mixed:uv          bit set, paragraph keys (below)
```

Version 2 is version 1 with `formatting` appended; every field before it is unchanged, and
every version 1 request is still valid. A decoder must check the fourth byte: version 1
records carry no `formatting`.

**Formatting** describes where the selection starts. For a collapsed selection it is what the
next typed text gets: the pending formatting chosen at the caret (a toggle before typing),
else the character before the caret, else the one after it. For a range it is the first
character's. "After the cascade" means document defaults, then the paragraph style chain,
then the character style chain, then the direct properties: what the ribbons show as pressed.
`paragraph.style` is the paragraph style id (absent: none applied), `paragraph.numbering` is
`{numId, level}` for a list item. A ribbon control shows the indeterminate state when its key
is set in the mixed sets: the effective value differs somewhere in the selected text
(`run_mixed`) or between the selected paragraphs (`paragraph_mixed`). Both sets are empty for
a collapsed selection.

| bit | run key | | bit | paragraph key |
|---|---|---|---|---|
| 0 | `style` | | 0 | `style` |
| 1 | `font` | | 1 | `alignment` |
| 2 | `size` | | 2 | `spacing` |
| 3 | `bold` | | 3 | `indent` |
| 4 | `italic` | | 4 | `borders` |
| 5 | `underline` | | 5 | `shading` |
| 6 | `strike` | | 6 | `keepWithNext` |
| 7 | `verticalAlign` | | 7 | `keepLines` |
| 8 | `color` | | 8 | `pageBreakBefore` |
| 9 | `highlight` | | 9 | `numbering` |
| 10 | `smallCaps` | | 10 | `outlineLevel` |
| 11 | `allCaps` | | | |

A bit past the last key is malformed.

`paint(pages)` returns the display list of those pages (`docs/display-list.md`).

## Tombstone collection

`gc(known)` removes the deleted units and delete operations that every listed peer has
seen, and returns what it removed:

```text
known     := count:uv (state_vector:bytes){count}   each peer's acknowledged stateVector()
collected := "CDG" 0x01 units:uv deletes:uv
```

The engine never collects on its own: only the collaboration session knows which peers exist
and what each has acknowledged. Call it right before storing a snapshot, when every peer is
caught up. It fails and changes nothing when `known` is empty, when updates wait for their
dependencies, when a listed peer holds operations this replica lacks (apply its diff first),
or while a composition is open. The document and the selection are unchanged; **the undo
history ends**, since undoing a deletion re-anchors text after units that collection may have
removed.

## Page setup

`SetSectionProperty` changes the page setup of every section from the anchor's to the head's
in one transaction and one undo step. The patch (`PageSetupPatch`) has four optional fields;
an absent one keeps the section's value, a present one replaces it whole:

```text
{ size?:        { width, height, orientation: "portrait" | "landscape" },
  orientation?: "portrait" | "landscape",
  margins?:     { top, bottom, left, right, header, footer, gutter },
  columns?:     { count, gap, separator: bool } }
```

Lengths are Pt100. `orientation` applies after `size`: it swaps width and height when the paper
has the other orientation, so the Orientation button sends `orientation` alone. Page
numbering is not part of the patch. The command fails and changes nothing when the patch sets
nothing, when the result is a page the model rejects (a size that is not positive, a negative
margin, zero columns), or when a selection end is in a footnote body.

## Other records

```text
position record   := position                        (hitTest(page, x, y) returns one)
awareness         := "CDA" 0x01 client:uv anchor:position head:position
peer_view         := "CDP" 0x01 client:uv caret:(0 | 1 page:uv rect)
                     count:uv (page:uv rect){count}
```

The relay forwards awareness records as they are; `peerView(record)` turns a peer's record
into rectangles on this replica's pages. Updates, snapshots and state vectors are
`cdoc-crdt`'s own formats: send `update` to peers, call `applyRemote` with theirs, and use
`stateVector` / `encodeDiff` to catch a replica up.
