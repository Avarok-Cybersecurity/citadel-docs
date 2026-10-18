// Step 1 of the protocol (README.md): opening the corpus through the product's own import, the
// full page count calibrated by an untimed open, stalled opens abandoned and redone.

import type { BrowserContext, Frame, Page } from "playwright";
import type { Adapter, Iterations } from "./types.js";
import { documentFrame, probe, reached, settled, sleep, until } from "./wait.js";

/** A 500-page import on a 2-vCPU runner; past this the run fails rather than waits. */
export const OPEN_TIMEOUT_MS = 900_000;
/** The page count unchanged this long after its last change is the product's full count. */
export const SETTLE_MS = 30_000;

const ns = (ms: number): number => {
  if (!Number.isFinite(ms) || ms < 0) throw new Error(`bad duration ${ms} ms`);
  return Math.round(ms * 1e6);
};

/** Runs `warmup + iterations` times, keeping the timed runs' results. */
export async function repeat<T>(it: Iterations, step: (last: boolean) => Promise<T>): Promise<T[]> {
  const kept: T[] = [];
  const total = it.warmup + it.iterations;
  for (let k = 0; k < total; k += 1) {
    const value = await step(k === total - 1);
    if (k >= it.warmup) kept.push(value);
  }
  return kept;
}

export interface Opened {
  readonly page: Page;
  readonly frame: Frame;
  readonly startEpochMs: number;
  readonly tabs: Page[];
}

async function openOnce(ctx: BrowserContext, adapter: Adapter, docx: string): Promise<Opened> {
  const first = await ctx.newPage();
  const { page, startEpochMs } = await adapter.open(first, docx);
  await page.bringToFront();
  const frame = await documentFrame(page, adapter.probe.frameUrl, OPEN_TIMEOUT_MS);
  return { page, frame, startEpochMs, tabs: [first, page] };
}

/** After a tab closes, the network is let go quiet before the next open: a load racing the closed
 * tab's cancelled requests was seen aborting the editor's scripts and hanging the open. */
const QUIET_MS = 3_000;

export const closeAll = async (o: Opened): Promise<void> => {
  for (const tab of new Set(o.tabs)) await tab.close({ runBeforeUnload: false });
  await sleep(QUIET_MS);
};

/** An open whose editor shows no page count at all this long after the start never loaded (its
 * scripts' requests were aborted); every product shows its first page within seconds. */
const STALL_MS = 60_000;
/** Abandoned opens tolerated per corpus size before the run fails. */
const MAX_ABANDONED = 2;

/** An open whose editor loaded; a stalled one is closed, noted in `abandoned`, and redone. */
async function openLoaded(ctx: BrowserContext, adapter: Adapter, docx: string, abandoned: string[]) {
  for (;;) {
    const o = await openOnce(ctx, adapter, docx);
    const shown = await reached(o.frame, 1, STALL_MS).catch(() => null);
    if (shown !== null) return o;
    await closeAll(o);
    abandoned.push(`${new Date().toISOString()}: no page count ${STALL_MS} ms after the start`);
    if (abandoned.length > MAX_ABANDONED) throw new Error(`${abandoned.length} opens stalled`);
  }
}

/** The open's stop: the frame showing the full count, or for an instrumented product the later of
 * that and the frame after its first paint (PaintStops). Both epoch ms. */
async function stopOf(o: Opened, adapter: Adapter, full: number) {
  const shown = (await reached(o.frame, full, OPEN_TIMEOUT_MS)).atEpochMs;
  if (adapter.paint.kind === "protocol") return { stop: shown, shown };
  const opened = adapter.paint.opened;
  const painted = await until("the first paint", OPEN_TIMEOUT_MS, () => opened(o.frame));
  return { stop: Math.max(shown, painted), shown };
}

/** Step 1: calibrates the full count untimed, then times opens to the frame that shows it. */
export async function open(ctx: BrowserContext, adapter: Adapter, it: Iterations, docx: string) {
  const abandoned: string[] = [];
  const calibration = await openLoaded(ctx, adapter, docx, abandoned);
  const full = await settled(calibration.frame, SETTLE_MS, OPEN_TIMEOUT_MS);
  const { counts } = await probe(calibration.frame);
  const growth = counts.map((c) => [c.total, Math.round(c.atEpochMs - calibration.startEpochMs)]);
  const visibility = await calibration.frame.evaluate(() => [document.visibilityState, document.hasFocus()]);
  const steps = growth.filter((g, i) => i === 0 || g[0] !== growth[i - 1]?.[0]);
  const seen = { full, visibility, growth: steps, abandoned };
  await closeAll(calibration);
  let kept: Opened | null = null;
  const shownOnly: number[] = [];
  const samples = await repeat(it, async (last) => {
    const o = await openLoaded(ctx, adapter, docx, abandoned);
    const { stop, shown } = await stopOf(o, adapter, full);
    if (last) kept = o;
    else await closeAll(o);
    shownOnly.push(ns(shown - o.startEpochMs));
    return ns(stop - o.startEpochMs);
  });
  if (kept === null) throw new Error("no open kept");
  // Warmup opens included, in order; the timed ones are the last `it.iterations`.
  return { samples, full, seen, shownOnly, opened: kept as Opened };
}

