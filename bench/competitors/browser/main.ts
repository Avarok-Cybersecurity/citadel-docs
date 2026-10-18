// The browser driver's I/O edge: one product per process, a fresh profile, Chromium as the pinned
// Playwright ships it, a 1920 x 1080 viewport. Writes a RawRun for `cdoc-bench summarize --cells
// browser`, plus notes (the page counts each size laid out to) and screenshots for the artifact.
//
// usage: node dist/browser/main.js --product ID --plan plan.json --corpus DIR --out raw.json
//          --notes notes.json --shots DIR --runner TEXT --commit SHA --date YYYY-MM-DD --run-url URL

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { env, exit } from "node:process";
import { parseArgs } from "node:util";
import { chromium, type Page } from "playwright";
import { googleDocs } from "../google-docs/adapter.js";
import { onlyoffice } from "../onlyoffice/adapter.js";
import { word } from "../word/adapter.js";
import { installProbe } from "./probe.js";
import { runProtocol } from "./protocol.js";
import type { BrowserPlan, Product, Provenance, RawRun } from "./types.js";
import { credentials, unconfigured } from "./unconfigured.js";

const PRODUCTS: readonly Product[] = [onlyoffice, word, googleDocs];
const FLAGS = ["product", "plan", "corpus", "out", "notes", "shots", "runner", "commit", "date", "run-url"] as const;
type Flags = Record<(typeof FLAGS)[number], string>;

function flags(): Flags {
  const options = Object.fromEntries(FLAGS.map((f) => [f, { type: "string" as const }]));
  const { values } = parseArgs({ options, strict: true });
  const read = (f: (typeof FLAGS)[number]): string => {
    const v = values[f];
    if (typeof v !== "string" || v === "") throw new Error(`missing --${f}`);
    return v;
  };
  return Object.fromEntries(FLAGS.map((f) => [f, read(f)])) as Flags;
}

const json = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`;

async function main(): Promise<void> {
  const a = flags();
  const product = PRODUCTS.find((p) => p.id === a.product);
  if (product === undefined) throw new Error(`unknown --product ${a.product}`);
  const plan = JSON.parse(await readFile(a.plan, "utf8")) as BrowserPlan;
  const provenance = (browser: string): Provenance => ({
    kind: "recorded",
    runner: a.runner,
    browser,
    commit: a.commit,
    date: a.date,
    run_url: a["run-url"],
  });
  const account = credentials(product, env);
  if (Array.isArray(account)) {
    console.log(`${product.tool}: not measured, missing ${account.join(", ")}`);
    const raw = unconfigured(product, plan, account, provenance("not launched"));
    await writeFile(a.out, json(raw));
    await writeFile(a.notes, json({ tool: product.tool, measured: false }));
    return;
  }
  const shots = a.shots;
  await mkdir(shots, { recursive: true });
  const browser = await chromium.launch({ headless: false, args: ["--window-size=1920,1080"] });
  const pw = createRequire(import.meta.url)("playwright/package.json") as { version: string };
  const browserName = `Chromium ${browser.version()} (Playwright ${pw.version}), headed under Xvfb`;
  const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 }, acceptDownloads: true });
  const adapter = product.create(account);
  await ctx.addInitScript(installProbe, adapter.probe);
  const shot = async (page: Page, name: string): Promise<void> => {
    await page.screenshot({ path: `${shots}/${product.id}-${name}.png` });
  };
  const first = await ctx.newPage();
  try {
    await adapter.prepare(first);
    const version = await adapter.version(first);
    const m = await runProtocol(ctx, adapter, plan, a.corpus, shot);
    const raw: RawRun = {
      tool: product.tool,
      version,
      samples: m.samples,
      bytes: m.bytes,
      frames: m.frames,
      unmeasured: m.unmeasured,
      provenance: provenance(browserName),
    };
    await writeFile(a.out, json(raw));
    await writeFile(a.notes, json({ tool: product.tool, measured: true, pageCounts: m.pageCounts }));
  } catch (e) {
    for (const [k, page] of ctx.pages().entries()) {
      await page.screenshot({ path: `${shots}/${product.id}-failure-${k}.png` }).catch(() => undefined);
    }
    throw e;
  } finally {
    await browser.close();
  }
}

main().catch((e: unknown) => {
  console.error(e instanceof Error ? (e.stack ?? e.message) : String(e));
  exit(1);
});
