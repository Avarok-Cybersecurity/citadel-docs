// The no-account path: without both secrets a product writes "not measured" for every cell and
// never reads a value; with both it gets them. A product that needs no account is never gated.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { BrowserPlan, Product, Provenance } from "./types.js";
import { credentials, unconfigured } from "./unconfigured.js";

const product = (secrets: Product["secrets"]): Product => ({
  id: "p",
  tool: "P",
  secrets,
  kind: "competitor",
  create: () => {
    throw new Error("an unconfigured product must not be created");
  },
});

const word = product({ email: "BENCH_X_EMAIL", password: "BENCH_X_PASSWORD" });

const plan: BrowserPlan = {
  cells: {
    "docx_open/100p": { warmup: 1, iterations: 5 },
    "scroll/500p": { warmup: 1, iterations: 3 },
  },
  phrases: { "100p": "a b c d" },
  paragraphs: { "100p": 7 },
};

const provenance: Provenance = {
  kind: "recorded",
  runner: "r",
  browser: "not launched",
  commit: "c",
  date: "2026-10-17",
  run_url: "u",
};

test("no secrets: both names are reported missing", () => {
  assert.deepEqual(credentials(word, {}), ["BENCH_X_EMAIL", "BENCH_X_PASSWORD"]);
});

test("a blank secret counts as missing", () => {
  const env = { BENCH_X_EMAIL: "a@b", BENCH_X_PASSWORD: "  " };
  assert.deepEqual(credentials(word, env), ["BENCH_X_PASSWORD"]);
});

test("both secrets: the account is returned", () => {
  const env = { BENCH_X_EMAIL: "a@b", BENCH_X_PASSWORD: "pw" };
  assert.deepEqual(credentials(word, env), { email: "a@b", password: "pw" });
});

test("a product without an account is never gated", () => {
  assert.equal(credentials(product(null), {}), null);
});

test("unconfigured: every cell is not measured, with the secrets named, and no number", () => {
  const raw = unconfigured(word, plan, ["BENCH_X_EMAIL", "BENCH_X_PASSWORD"], provenance);
  assert.deepEqual(Object.keys(raw.unmeasured).sort(), [
    "docx_open/100p",
    "scroll/500p",
    "scroll_long_frames/500p",
  ]);
  for (const cell of Object.values(raw.unmeasured)) {
    assert.equal(cell.status, "not measured");
    assert.match(cell.reason, /no test account configured.*BENCH_X_EMAIL and BENCH_X_PASSWORD/);
  }
  assert.deepEqual([raw.samples, raw.bytes, raw.frames], [{}, {}, {}]);
  assert.equal(raw.version, "not measured");
});
