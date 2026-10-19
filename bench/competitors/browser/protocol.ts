// The wave 3 browser protocol (bench/competitors/README.md is its text; this is its code), the
// same for every product: open, keystroke to paint, scroll, export, memory. Everything
// product-specific goes through an `Adapter`. Iterations come from the plan, never from here.

import { hrtime } from "node:process";
import type { BrowserContext, Frame, Page } from "playwright";
import type { Adapter, BrowserPlan, FrameCount, Iterations, Unmeasured } from "./types.js";
import { closeAll, open, OPEN_TIMEOUT_MS, type Opened, repeat, SETTLE_MS } from "./open.js";
import { nextDownload, probe, settled, sleep, until } from "./wait.js";

/** One export, or one keystroke's paint, past which the run fails. */
const STEP_TIMEOUT_MS = 300_000;
/** One keystroke every 100 ms (about a typist's pace), so a keystroke's deferred work does not
 * land in the next one's frames. */
const KEY_GAP_MS = 100;
/** A 60 Hz frame. */
const LONG_FRAME_NS = 16_700_000;
/** A scroll pass that has not reached the end after this many frames has failed. */
const MAX_SCROLL_FRAMES = 200_000;

export interface Measured {
  readonly samples: Record<string, number[]>;
  readonly bytes: Record<string, number[]>;
  readonly frames: Record<string, FrameCount>;
  readonly unmeasured: Record<string, Unmeasured>;
  /** What the run saw, by corpus size (`100p`): the full count, how the count grew during the
   * calibration open (total, ms after the start), and whether the frame was visible. */
  readonly notes: Record<string, unknown>;
}

interface ScrollWindow {
  __cdocScrollStep?: () => boolean;
  __cdocScrollTop?: () => void;
}

const ns = (ms: number): number => {
  if (!Number.isFinite(ms) || ms < 0) throw new Error(`bad duration ${ms} ms`);
  return Math.round(ms * 1e6);
};

function iterationsOf(plan: BrowserPlan, cell: string): Iterations {
  const it = plan.cells[cell];
  if (it === undefined) throw new Error(`the plan has no ${cell}`);
  return it;
}


/** Step 2: one keystroke at a time, alternating type and delete. For an instrumented product
 * each sample is the later of the second frame and the frame after the edit's paint; the second
 * frame alone goes to `secondFrame`, warmup included. */
async function keystrokes(o: Opened, adapter: Adapter, it: Iterations, secondFrame: number[]) {
  let typed = 0;
  return repeat(it, async () => {
    await o.frame.evaluate(() => {
      const state = (window as unknown as { __cdocProbe?: { keys: number[] } }).__cdocProbe;
      state?.keys.splice(0);
    });
    await o.page.keyboard.press(typed % 2 === 0 ? "x" : "Backspace");
    typed += 1;
    const ms = await until("the keystroke's paint", STEP_TIMEOUT_MS, async () => {
      const { keys } = await probe(o.frame);
      if (keys.length > 1) throw new Error(`one keystroke stamped ${keys.length} times`);
      return keys[0] ?? null;
    });
    secondFrame.push(ns(ms));
    let stop = ms;
    if (adapter.paint.kind === "instrumented") {
      const read = adapter.paint.keystroke;
      stop = Math.max(ms, await until("the keystroke's paint", STEP_TIMEOUT_MS, () => read(o.frame)));
    }
    await sleep(KEY_GAP_MS);
    return ns(stop);
  });
}

/** Step 3: from page 1, one viewport per frame to the end; every frame's delta, in order. */
async function scroll(o: Opened, adapter: Adapter, it: Iterations): Promise<number[]> {
  await adapter.installScroll(o.frame);
  const passes = await repeat(it, async () => {
    await o.frame.evaluate(() => (window as unknown as ScrollWindow).__cdocScrollTop?.());
    await sleep(SETTLE_MS);
    return o.frame.evaluate(
      (max: number) =>
        new Promise<number[]>((resolve, reject) => {
          const w = window as unknown as ScrollWindow;
          const step = w.__cdocScrollStep;
          if (step === undefined) return reject(new Error("no __cdocScrollStep"));
          const deltas: number[] = [];
          let prev: number | null = null;
          const frame = (t: number): void => {
            if (prev !== null) deltas.push(t - prev);
            prev = t;
            if (step()) resolve(deltas);
            else if (deltas.length > max) reject(new Error(`no end after ${max} frames`));
            else requestAnimationFrame(frame);
          };
          requestAnimationFrame(frame);
        }),
      MAX_SCROLL_FRAMES,
    );
  });
  return passes.flat().map(ns);
}

/** Step 4: the product's own PDF export, command to `download.path()`. */
async function exports(ctx: BrowserContext, o: Opened, adapter: Adapter, it: Iterations) {
  return repeat(it, async () => {
    await adapter.openExport(o.page, o.frame);
    const download = nextDownload(ctx, o.page, STEP_TIMEOUT_MS);
    const start = hrtime.bigint();
    await adapter.exportCommand(o.page, o.frame);
    const path = await (await download).path();
    const elapsed = hrtime.bigint() - start;
    if (path === null) throw new Error("the export's download has no file");
    return Number(elapsed);
  });
}

/** Step 5: `measureUserAgentSpecificMemory()` where the document frame is isolated. */
async function memory(o: Opened, it: Iterations): Promise<number[] | Unmeasured> {
  const isolated = await o.frame.evaluate(() => crossOriginIsolated);
  if (!isolated) {
    const origin = new URL(o.frame.url()).origin;
    return {
      status: "not measured",
      reason:
        `the document frame (${origin}) is not cross-origin isolated (crossOriginIsolated is ` +
        "false), so performance.measureUserAgentSpecificMemory() is unavailable; no estimate",
    };
  }
  return repeat(it, () =>
    o.frame.evaluate(async () => {
      type Measure = () => Promise<{ bytes: number }>;
      const p = performance as Performance & { measureUserAgentSpecificMemory?: Measure };
      if (p.measureUserAgentSpecificMemory === undefined) throw new Error("no memory API");
      return (await p.measureUserAgentSpecificMemory()).bytes;
    }),
  );
}

/** Every cell of the plan, one corpus size at a time, smallest first. */
export async function runProtocol(
  ctx: BrowserContext,
  adapter: Adapter,
  plan: BrowserPlan,
  corpus: string,
  shot: (page: Page, name: string) => Promise<void>,
): Promise<Measured> {
  const m: Measured = { samples: {}, bytes: {}, frames: {}, unmeasured: {}, notes: {} };
  const sizes = Object.keys(plan.cells)
    .filter((c) => c.startsWith("docx_open/"))
    .map((c) => c.slice("docx_open/".length))
    .sort((a, b) => Number.parseInt(a, 10) - Number.parseInt(b, 10));
  for (const size of sizes) {
    const phrase = plan.phrases[size];
    const paragraph = plan.paragraphs[size];
    if (phrase === undefined || paragraph === undefined) throw new Error(`the plan has no keystroke target for ${size}`);
    const opened = await open(ctx, adapter, iterationsOf(plan, `docx_open/${size}`), `${corpus}/${size}.docx`);
    const o = opened.opened;
    m.samples[`docx_open/${size}`] = opened.samples;
    m.notes[size] = opened.seen;
    await settled(o.frame, SETTLE_MS, OPEN_TIMEOUT_MS);
    await o.frame.evaluate(() => {
      const state = (window as unknown as { __cdocProbe?: { watching: boolean } }).__cdocProbe;
      if (state !== undefined) state.watching = false;
    });
    await shot(o.page, `${size}-opened`);
    await adapter.placeCaret(o.page, o.frame, { phrase, paragraph });
    await shot(o.page, `${size}-caret`);
    const keys = iterationsOf(plan, `layout_keystroke/${size}`);
    const secondFrame: number[] = [];
    m.samples[`layout_keystroke/${size}`] = await keystrokes(o, adapter, keys, secondFrame);
    if (adapter.paint.kind === "instrumented") {
      // The protocol's own stops, kept beside the cells (which take the later paint).
      m.notes[`${size}/protocol-stops`] = { docx_open_ns: opened.shownOnly, layout_keystroke_ns: secondFrame };
    }
    await shot(o.page, `${size}-typed`);
    const pdf = iterationsOf(plan, `docx_to_pdf/${size}`);
    m.samples[`docx_to_pdf/${size}`] = await exports(ctx, o, adapter, pdf);
    if (plan.cells[`scroll/${size}`] !== undefined) {
      const deltas = await scroll(o, adapter, iterationsOf(plan, `scroll/${size}`));
      m.samples[`scroll/${size}`] = deltas;
      const over = deltas.filter((d) => d > LONG_FRAME_NS).length;
      m.frames[`scroll_long_frames/${size}`] = { over, of: deltas.length };
      await shot(o.page, `${size}-scrolled`);
    }
    if (plan.cells[`memory/${size}`] !== undefined) {
      const reading = await memory(o, iterationsOf(plan, `memory/${size}`));
      if (Array.isArray(reading)) m.bytes[`memory/${size}`] = reading;
      else m.unmeasured[`memory/${size}`] = reading;
    }
    await closeAll(o);
  }
  return m;
}
