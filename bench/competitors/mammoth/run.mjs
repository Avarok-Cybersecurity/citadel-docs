// mammoth opening the cdoc-bench `.docx` fixtures (docx to HTML, its only open path), timed
// with process.hrtime.bigint(). Writes raw samples; `cdoc-bench summarize` computes the
// statistics the way it computes ours.
//
// usage: node run.mjs OUT.json CORPUS_DIR PAGES:WARMUP:ITERATIONS [PAGES:WARMUP:ITERATIONS ..]

import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import mammoth from "mammoth";

const require = createRequire(import.meta.url);
const version = require("mammoth/package.json").version;

const [out, corpus, ...runs] = process.argv.slice(2);
if (!out || !corpus || runs.length === 0) {
  console.error("usage: node run.mjs OUT.json CORPUS_DIR PAGES:WARMUP:ITERATIONS ..");
  process.exit(2);
}

const samples = {};
const unmeasured = {};
for (const run of runs) {
  const [pages, warmup, iterations] = run.split(":").map((n) => Number.parseInt(n, 10));
  if (![pages, warmup, iterations].every(Number.isInteger) || iterations < 1) {
    throw new Error(`bad run ${run}`);
  }
  const buffer = await readFile(`${corpus}/${pages}p.docx`);
  const once = async () => {
    const result = await mammoth.convertToHtml({ buffer });
    if (result.value.length === 0) throw new Error(`${pages}p: mammoth produced no HTML`);
  };
  for (let k = 0; k < warmup; k += 1) await once();
  const times = [];
  for (let k = 0; k < iterations; k += 1) {
    const start = process.hrtime.bigint();
    await once();
    times.push(Number(process.hrtime.bigint() - start));
  }
  samples[`docx_open/${pages}p`] = times;
  for (const family of ["layout_full", "pdf_export", "docx_to_pdf"]) {
    unmeasured[`${family}/${pages}p`] = {
      status: "not comparable",
      reason: "mammoth converts to HTML; it has no page layout or PDF export",
    };
  }
}

await writeFile(out, JSON.stringify(
    {
      tool: "mammoth",
      version,
      samples,
      bytes: {},
      frames: {},
      unmeasured,
      provenance: { kind: "this run" },
    },
    null,
    2,
  ),);
