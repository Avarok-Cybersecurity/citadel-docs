# Editor records, version 1

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
  (`RunProperties`, `ParagraphProperties`, `Comment`); absent optional fields are omitted.
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
result := "CDE" 0x01
          update:bytes                    the update to relay; empty when nothing changed
                                          (always empty for applyRemote)
          count:uv page:uv{count}         pages to repaint, ascending
          page_count:uv
          anchor:position head:position   the selection
          caret:(0 | 1 page:uv rect)      the caret (zero width, line height)
          count:uv (page:uv rect){count}  the selection highlight, one rect per line
```

`paint(pages)` returns the display list of those pages (`docs/display-list.md`).

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
