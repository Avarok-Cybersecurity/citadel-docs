# Editor records, version 4

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
  (`RunProperties`, `ParagraphPatch`, `PageSetupPatch`, `Comment`); absent optional fields
  are omitted. Lengths in values are Pt100 integers.
- Every record is read to its end: trailing bytes are an error, as is a truncated record.
- **Client ids** are `u64`, a `BigInt` in JavaScript (`EditorHandle.blank(1n, fonts)`).
  Client `0` is reserved: it authors the blank document's base items, so every
  `EditorHandle.blank(client)` is the same document and blanks opened apart converge. Opening
  any replica as client `0` fails (`client 0 is reserved for the blank document's base items`).

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
| `0x08` | SetRunProperty | `patch:value` (`RunPatch`, below: absent keeps, a value sets, `null` clears back to the style's; a version 1 `RunProperties` payload reads as the patch that sets its fields) |
| `0x09` | SetParagraphProperty | `patch:value` (`ParagraphPatch`, below: absent keeps, a value sets, `null` clears; `spacing` and `indent` patched field by field) |
| `0x0A` | SetParagraphStyle | `style:str` |
| `0x0B` | SetNumbering | `num_id:uv level:u8` |
| `0x0C` | ClearNumbering | |
| `0x19` | SetNumberingKind | `kind:u8 level:u8` (kind 0 bullet, 1 decimal, 2 lower letter, 3 upper letter, 4 lower roman, 5 upper roman; version 3) |
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
| `0x25` | InsertFootnote, id picked by the editor | (the id comes back in `created`; version 3) |
| `0x26` | InsertImage at its own size | `content_id:str` (below; version 3) |
| `0x22` | InsertLink | `href:str` (links the selection, or inserts the URL as linked text) |
| `0x23` | RemoveLink | |
| `0x24` | InsertComment | `comment:value` (`Comment`) |
| `0x30` | Replace | `from:position to:position text:str` |
| `0x31` | SelectAll | (the body: from its first caret gap, past the first section's headers and footers, to the last before the footnotes) |
| `0x32` | SetSelection | `anchor:position head:position` |
| `0x33` | Cut | (deletes the selection as one undo step; a caret fails. Copy its text first: the result's `selection_text`, or `selectionText()` past the cap; version 4) |
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
| `0x60` | SetViewport | `count:uv page:uv{count}` (the pages on screen; not a document command, no undo step; version 4) |

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
result := "CDE" 0x04
          update:bytes                    the update to relay; empty when nothing changed
                                          (always empty for applyRemote)
          count:uv page:uv{count}         pages to repaint, ascending
          page_count:uv
          anchor:position head:position   the selection
          caret:(0 | 1 page:uv rect)      the caret (zero width, line height)
          count:uv (page:uv rect){count}  the selection highlight, one rect per line, on
                                          the viewport's and the repainted pages once
                                          `0x60` named a viewport (on every page before)
          formatting                      (new in version 2)
          count:uv (kind:u8 id:uv){count} created (new in version 3): what the request
                                          made under ids the editor picked; kind 0 a
                                          footnote (`0x25`), 1 a numbering instance
                                          (`0x19`, when no definition could be reused)
          section:value                   PageSetup of the caret's section (new in
                                          version 4): size {width, height, orientation},
                                          margins {top, bottom, left, right, header,
                                          footer}, columns {count, gap, separator},
                                          page numbering
          complete:u8 text:str            the selection's plain text (new in version 4):
                                          complete 1 and the text, or, past 64 KiB of
                                          UTF-8, complete 0 and an empty str (the length
                                          byte 0 is always there; a decoder refuses text
                                          with complete 0)

formatting := run:value                   RunProperties after the cascade
              run_direct:value            RunProperties set directly on the run
              paragraph:value             ParagraphProperties after the cascade
              paragraph_direct:value      ParagraphProperties set directly on the paragraph
              run_mixed:uv                bit set, run keys (below)
              paragraph_mixed:uv          bit set, paragraph keys (below)
```

Version 2 is version 1 with `formatting` appended, version 3 appends `created`, and version 4
appends `section` and the selection text; every field before them is unchanged, and every
earlier request is still valid. The decoder reads version 4 only and refuses any other by name
(`result record version 3; this build reads only version 4`).

**Selection text** is what Copy and Cut put on the clipboard, in the record because a copy
event must set the clipboard synchronously: characters as they are, `\n` where a paragraph
starts and for a break, `\t` for a tab; images, fields and other objects give nothing, and
headers, footers and footnote bodies inside the selection are skipped. Empty for a caret.
Past `MAX_SELECTION_TEXT_BYTES` (64 KiB) the record says incomplete and its `text` is the empty
`str` (a zero length, still present; any text with `complete` 0 is refused): call
`selectionText()` (no cap) and set the clipboard from it outside the event, or ask the user
to copy again. Paste is `InsertText`.

**Rectangles** of a long selection cost a line each, so once the UI sends `SetViewport` with
the pages on screen (again whenever that changes) a result carries them for those pages and
the ones it repaints. For a page scrolled into view call `selectionRects(page)`:

```text
rects := "CDR" 0x01 count:uv rect{count}
```

**Section** is the setup Page Setup shows; send `size` in `SetSectionProperty` with the
current orientation from here to keep it.

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
| 11 | `allCaps` | | 11 | `suppressAutoHyphens` |
| 12 | `kern` | | | |
| 13 | `ligatures` | | | |
| 14 | `lang` | | | |

A bit past the last key is malformed.

`paint(pages)` returns the display list of those pages (`docs/display-list.md`).

## Run patches

`RunPatch` has the twelve `RunProperties` fields, each absent (keep), a value (set) or `null`
(clear the direct property, back to what the paragraph and character styles give: no
highlight, the style's colour). Clear Formatting is the patch with every field `null`
(`RunPatch::clear_all`). A patch that names nothing fails. At a caret the patch applies to
the next typed text.

## Lists

`SetNumberingKind` makes the selected paragraphs items of one of Word's built-in list
templates at `level` (`0..=8`): nine levels, each a further 0.5 in with a 0.25 in hanging
label; bullets `•`, `◦`, `▪` repeating; numbered kinds cycle three formats (decimal: `1.`,
`a.`, `i.`; lower letter: `a.`, `i.`, `1.`; upper letter: `A.`, `a.`, `i.`; lower roman:
`i.`, `1.`, `a.`; upper roman: `I.`, `A.`, `1.`). It reuses the document's definition with
exactly those levels and its first instance that overrides no start; otherwise it adds a
definition and an instance in the same undo step and reports the instance in `created`. Use
`SetNumbering` with that id to continue the same list elsewhere.

Ids the editor picks (footnotes, list definitions and instances) are derived from the
replica's client id and clock, so two replicas picking at once get different ids; they are
in `1..2^31`.

Undoing a footnote's insertion removes its reference and keeps its body, unreferenced, as
deleting the reference does; an unreferenced body is not drawn. Redo, or a peer restoring the
reference concurrently (by undoing their own deletion of it), finds the body the reference
names, where deleting the body would have left a reference to nothing.

## Styles

`styles()` returns the document's own style sheet, for the gallery:

```text
styles   := "CDS" 0x01 count:uv style{count}
style    := id:str name:str kind:u8 based_on:(0 | 1 id:str) run:value paragraph:value
kind     := 0 paragraph | 1 character | 2 table
```

`run` and `paragraph` are the `RunProperties` and `ParagraphProperties` the style resolves to
through its `based_on` chain and the document defaults: what a gallery tile previews. A
character style's `paragraph` is the defaults. The list is in style-sheet order. Call it
again when the result's `update` changed the styles (an imported or remote style sheet).

## Images

`InsertImage` names an image by content id; `addImage(bytes, maxBytes)` is how the editor
gets one. It returns:

```text
added_image := "CDI" 0x01 content_id:str kind:u8 width:uv height:uv
kind        := 0 PNG | 1 JPEG
```

- The type is read from the file's signature, never from a declared MIME type. PNG and JPEG
  are accepted; anything else fails by name (`unsupported image type: GIF`, `WebP`, `BMP`,
  `TIFF`, `SVG`, `unknown`), as does a PNG or JPEG whose header is cut short or has no pixels.
- `maxBytes` is the caller's limit, inclusive; a larger file fails and nothing is held.
- `content_id` is `sha256-` followed by the lowercase hex SHA-256 of the file. Every peer
  derives the same id from the same bytes, so store the bytes in the workspace's content store
  under that id (and check fetched bytes against it) before relaying the `InsertImage` update.
- `width` and `height` are pixels. Send the display size with `InsertImage` (`0x20`, Pt100),
  or use `0x26` to insert an image the editor holds at its own size: 96 pixels per inch
  (0.75 pt a pixel), scaled down, keeping its aspect ratio, to the width of one column of the
  caret's section (page width less the left and right margins, less the gaps, divided by the
  columns; table cells are not considered). Never scaled up. An image the editor does not
  hold fails by name.

The canvas painter reads the bytes of an image the editor holds with `image(contentId)`, and
`images()` lists them all for warming a cache:

```text
image  := "CDB" 0x01 mime:str bytes:bytes
images := "CDL" 0x01 count:uv (content_id:str mime:str byte_len:uv){count}
```

An id the editor does not hold fails by name (`no image with content id "..."`); fetch those
from the content store. Draw anything but `image/png` and `image/jpeg` as a placeholder box.

The editor keeps the files it was given for the session (`Editor::images`, an `ImagePort` for
PDF, DOCX and ODT export), and those of a file it was opened on (below). Images that arrive
from peers are the content store's.

## Files

`EditorHandle.fromDocument(bytes, kind, optionsJson, client, fonts, maxImageBytes)` opens a
new document (a new replica written by `client`; peers open its snapshot) on a file, and
returns an `ImportedEditor`: `reportJson()` is `{"report", "problems", "kind"}`, the import
shapes of `docx_to_pm` without `pm`, and `editor()` hands over the `EditorHandle`.

| kind   | file                                   | options JSON                                      |
| ------ | -------------------------------------- | ------------------------------------------------- |
| `docx` | `.docx`, `.dotx`; `.docm`/`.dotm` without macros | `{}`                                    |
| `odt`  | `.odt`, `.ott`                         | `{}`                                              |
| `md`   | UTF-8 Markdown                         | `{"textWidth", "imageWidth", "imageHeight"}` (Pt100) |
| `txt`  | UTF-8 text                             | `{"singleNewline": "break" \| "space"}`           |

Each embedded image is renamed to its content id in the document; store its bytes under
that id before peers open the snapshot. An image larger than `maxImageBytes` fails the open by
name. An image of a type the editor does not draw (GIF, WebP, BMP, TIFF, EMF, ...) never
does: it is kept with its declared MIME type and its extent from the file, drawn as a
placeholder box (PDF and SVG; the canvas sees an ordinary image op and should do the same for
anything but PNG and JPEG), exported unchanged to DOCX and ODT, and named in the report
(`{"part": "images", "what": "image type GIF not rendered", "severity": "minor", ...}`; every
report entry carries a severity and a label, see [loss-report.md](loss-report.md)). Only
`addImage`, a user's upload, refuses those types. A declared type outside the carried image types (`image/png`, `jpeg`, `gif`,
`bmp`, `tiff`, `webp`, `emf`/`x-emf`, `wmf`/`x-wmf`; SVG is not one, it can script) is held
as `application/octet-stream` instead, so it is never served or exported as what it claimed
to be, and the report says `image type "image/svg+xml" kept as an opaque file, not rendered`.

`EditorHandle.export(kind, optionsJson, images)` writes the live replica through the same
writers as `pm_to_docx`, `pm_to_odt`, `pm_to_md`, `pm_to_txt` and returns a `FileExport`
(`bytes()`, `reportJson()` = `{"losses": [...]}`). Image bytes come from the session's own
images, then from `images` (an `ImageStore` holding the content store's).

No file written from a document carries a footnote nothing references (an undone
footnote's body, which the session keeps for redo): every DOCX, ODT, Markdown and text export
drops them first (`Document::prune_unreferenced_footnotes`), and a PDF is drawn from the
layout, which never draws one.

| kind   | options JSON                          | bytes                              |
| ------ | ------------------------------------- | ---------------------------------- |
| `docx` | `{"kind": "document" \| "template"}`  | `.docx` / `.dotx`                  |
| `odt`  | `{"kind": "document" \| "template"}`  | `.odt` / `.ott`                    |
| `md`   | `{}`                                  | UTF-8 Markdown                     |
| `txt`  | `{}`                                  | UTF-8 text                         |
| `pdf`  | `{"now": <RFC 3339>}`                 | PDF of the editor's own layout; its report names each placeholder image (`"part": "pdf"`) |

Options are exact: a missing or unknown field fails, as does an unknown kind.

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

## Chunked diffs

`encodeDiffChunked(sv, maxBytes)` is `encodeDiff(sv)` for a log that refuses updates over a
size (the kernel's `max_update_bytes`): the same operations, cut into updates of at most
`maxBytes` bytes each, to append and apply in order. `sv` is a peer's `stateVector()`, or
`null` for a peer that holds nothing: an empty log, into which this publishes the replica's
whole state (an imported document) as ordinary updates.

```text
chunks := "CDK" 0x01 count:uv (update:bytes){count}   in the order they are applied
```

Each update is a complete one (`cdoc_crdt` format version 2) and names no unit that only a
later one introduces, so a peer applying them in order integrates each fully; applied out of
order, an update waits in the replica's buffer as any early update does. Text, tombstone runs
and skips are cut on unit boundaries; an atom, mark, attribute write or delete that does not fit
in `maxBytes` on its own is refused naming it, as is a limit below an update's frame. Nothing is
ever returned over the limit.

## Paragraph patches

`SetParagraphProperty` takes a `ParagraphPatch`: the fields of `ParagraphProperties`, each in
one of three states the value encoding carries as is.

| in the value | effect on each selected paragraph |
|---|---|
| field absent | keeps its direct value |
| a value | sets it |
| `null` (tag 0) | clears it, so the style's value shows |

`spacing` (`before`, `after`, `line`) and `indent` (`left`, `right`, `firstLine`) are patches
of their own: `{spacing: {line: …}}` changes the line spacing and keeps `before` and `after`;
`{spacing: {before: null}}` clears `before` alone; `{spacing: null}` clears the whole group, as
does clearing its last field. `borders` and `shading` are replaced whole. A version 1 payload
(a `ParagraphProperties`, which never holds `null`) reads as the patch that sets exactly its
fields; the one change is that its `spacing` and `indent` no longer clear the fields they omit.

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
