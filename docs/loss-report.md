# Loss reports

Every import and export returns a report of what the other format could not carry. Each entry
has a **severity** and a plain-language **label**, so a user can be told what matters without
reading element names. Both come from one table, `cdoc_model::loss`; no reader or writer
picks them itself.

## Shape

```json
{"losses": [
  {"part": "word/styles.xml", "what": "w:rPr/w:szCs", "count": 15,
   "severity": "info", "label": "Language and other-script settings"},
  {"part": "word/vbaProject.bin", "bytes": 12800,
   "severity": "info", "label": "Macros removed"}
]}
```

`part`, `what`, `count` and `bytes` are as before. `severity` and `label` are new, and they
are added to every entry. A consumer that ignores them sees the old shape.

## Tiers

| severity | meaning | examples |
|---|---|---|
| `info` | Nothing a reader of the document sees changes. | document properties; application settings; revision ids, proofing marks, bookmarks; language tags and East Asian or complex-script variants; parts nobody reads (`docProps/`, themes, the font table, thumbnails); a missing setting filled in the way word processors do; a reference to nothing tidied away; macros removed |
| `minor` | All the content is there, but some formatting, layout or structure is approximated or not kept. | a border style approximated as single; an unsupported paragraph or run property; a floating image placed inline; a list item's further paragraphs read as items; tracked changes accepted; a field kept as its text |
| `major` | Visible content is gone or changed: text, images, comments, notes or a part that may hold them. | a drawing that is not a picture; an image whose file is missing; a field that never ends; an unknown element inside a run or paragraph; a content part not read; comments or headers dropped by a Markdown export |

## How a loss is classified

1. An exact sentence, such as `a field that never ends`.
2. `part not read`, classified by the part's name. Parts with only metadata, application
   state, previews, fonts or themes are `info`. Any other part is `major`.
3. A sentence with a value in it, such as `field PAGEREF; its result is kept as text`.
4. An element or attribute path, such as `w:rPr/w:szCs` or `style:style@style:class`:
   - a leaf only an editor uses, or one for another script, is `info`, whatever holds it;
   - so is anything inside settings or document properties;
   - any other attribute, and any element of a formatting container (a `…Pr`, a
     `…-properties`, a style or list definition), is `minor`;
   - anything else is part of the content, so it is `major`.
5. Anything else is `major`, with the label "Content not carried". An unexpected loss is
   never hidden.

`crates/cdoc-model/tests/loss_coverage.rs` scans every reader and writer for the sentences
it can record, filling each `format!` placeholder with an element name. It fails when one of
them reaches rule 5. A new loss sentence therefore needs its row in the table before it can
merge.

## What the fixtures show

- The committed benchmark corpus (`crates/cdoc-bench/corpus`, both `.docx` and `.md`) imports
  with no `minor` or `major` loss (`cdoc-bench/tests/corpus_losses.rs`).
- None of the foreign `.docx` and `.odt` fixtures loses visible content: none has a `major`
  loss. Their unread parts and refused macros are `info` (`loss_tiers.rs` in cdoc-docx and
  cdoc-odt).
- Their `minor` entries are real simplifications: list label styles, the ODF defaults
  LibreOffice writes for line numbering, hyphenation and writing mode, and table style
  properties.

The readers report an unmodelled attribute whatever its value. So an attribute that holds
its format's default value (`text:number-lines="false"`, `style:writing-mode="lr-tb"`) is
still a `minor` entry, although nothing is lost. Not reporting default values is a reader
change, not a severity change.
