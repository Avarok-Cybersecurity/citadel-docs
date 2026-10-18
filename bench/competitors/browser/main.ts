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
import { ours } from "./ours/adapter.js";
import { onlyoffice } from "../onlyoffice/adapter.js";
import { installProbe } from "./probe.js";
import { runProtocol } from "./protocol.js";
import type { BrowserPlan, Product, Provenance, RawRun, Recorded } from "./types.js";

const PRODUCTS: readonly Product[] = [ours, onlyoffice];
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

/** A full commit id from the environment (ours' run.sh sets both), or the run stops. */
function commitFrom(name: string): string {
  const v = env[name] ?? "";
  if (!/^[0-9a-f]{40}$/.test(v)) throw new Error(`${name} is not a full commit id (ours/run.sh sets it)`);
  return v;
}

/** Ours names the UI commit built and the engine commit it vendors; a competitor, where it ran. */
function provenanceOf(product: Product, recorded: Recorded): Provenance {
  if (product.kind === "competitor") return { kind: "recorded", ...recorded };
  return {
    kind: "recorded ours",
    ...recorded,
    ui_commit: commitFrom("CDOC_UI_COMMIT"),
    engine_commit: commitFrom("CDOC_ENGINE_COMMIT"),
  };
}

async function main(): Promise<void> {
  const a = flags();
  const product = PRODUCTS.find((p) => p.id === a.product);
  if (product === undefined) throw new Error(`unknown --product ${a.product}`);
  const plan = JSON.parse(await readFile(a.plan, "utf8")) as BrowserPlan;
  const provenance = (browser: string): Provenance =>
    provenanceOf(product, { runner: a.runner, browser, commit: a.commit, date: a.date, run_url: a["run-url"] });
  provenance("not launched"); // ours without its commits stops here, before anything is measured
  const shots = a.shots;
  await mkdir(shots, { recursive: true });
  const browser = await chromium.launch({ headless: false, args: ["--window-size=1920,1080"] });
  const pw = createRequire(import.meta.url)("playwright/package.json") as { version: string };
  const browserName = `Chromium ${browser.version()} (Playwright ${pw.version}), headed under Xvfb`;
  const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 }, acceptDownloads: true });
  const adapter = product.create();
  await ctx.addInitScript(installProbe, adapter.probe);
  // The browser's own account of every page, for the artifact: console, failed requests, crashes.
  const log: string[] = [];
  const stamp = (line: string): number => log.push(`${new Date().toISOString()} ${line}`);
  ctx.on("page", (p) => {
    stamp(`page opened`);
    p.on("console", (m) => stamp(`console ${m.type()} ${m.text().slice(0, 300)}`));
    p.on("requestfailed", (r) => stamp(`requestfailed ${r.url().slice(0, 200)} ${r.failure()?.errorText ?? ""}`));
    p.on("crash", () => stamp("page crashed"));
    p.on("close", () => stamp("page closed"));
  });
  const shot = async (page: Page, name: string): Promise<void> => {
    await page.screenshot({ path: `${shots}/${product.id}-${name}.png` });
  };
  const first = await ctx.newPage();
  try {
    await adapter.prepare(first);
    const version = await adapter.version(first);
    await first.close();
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
    await writeFile(a.notes, json({ tool: product.tool, measured: true, notes: m.notes }));
  } catch (e) {
    for (const [k, page] of ctx.pages().entries()) {
      await page.screenshot({ path: `${shots}/${product.id}-failure-${k}.png` }).catch(() => undefined);
    }
    throw e;
  } finally {
    await writeFile(`${shots}/${product.id}-browser.log`, `${log.join("\n")}\n`);
    await browser.close();
  }
}

main().catch((e: unknown) => {
  console.error(e instanceof Error ? (e.stack ?? e.message) : String(e));
  exit(1);
});
