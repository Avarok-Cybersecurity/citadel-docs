// Waiting on the page from Node: polling the probe's state (never a `requestAnimationFrame` loop
// in the page, which would add work to every frame being measured), with explicit timeouts.

import type { BrowserContext, Download, Frame, Page } from "playwright";
import type { PageCountStamp, ProbeState, ProbeWindow } from "./probe.js";

/** How often Node reads the probe. Reading costs the page one small task per poll. */
export const POLL_MS = 250;

export const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

export async function probe(frame: Frame): Promise<ProbeState> {
  const state = await frame.evaluate(() => (window as unknown as ProbeWindow).__cdocProbe ?? null);
  if (state === null) throw new Error(`no probe in ${frame.url()}`);
  return state;
}

/** Polls `read` until it returns a value, or fails after `timeoutMs` naming `what`. */
export async function until<T>(what: string, timeoutMs: number, read: () => Promise<T | null>): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await read();
    if (value !== null) return value;
    if (Date.now() > deadline) throw new Error(`timed out after ${timeoutMs} ms waiting for ${what}`);
    await sleep(POLL_MS);
  }
}

/** The frame holding the document, once its probe is installed. */
export function documentFrame(page: Page, frameUrl: string, timeoutMs: number): Promise<Frame> {
  return until(`a frame at ${frameUrl}`, timeoutMs, async () => {
    for (const frame of page.frames()) {
      if (!frame.url().includes(frameUrl)) continue;
      const state = await frame
        .evaluate(() => (window as unknown as ProbeWindow).__cdocProbe?.isDocumentFrame ?? false)
        .catch(() => false);
      if (state) return frame;
    }
    return null;
  });
}

/** The first page-count stamp showing `total` pages or more. */
export function reached(frame: Frame, total: number, timeoutMs: number): Promise<PageCountStamp> {
  return until(`the page count to reach ${total}`, timeoutMs, async () => {
    const { counts } = await probe(frame);
    return counts.find((c) => c.total >= total) ?? null;
  });
}

/** The page count once it has not changed for `quietMs`: the product's full count. */
export async function settled(frame: Frame, quietMs: number, timeoutMs: number): Promise<number> {
  let seen = -1;
  let since = Date.now();
  return until(`the page count to settle for ${quietMs} ms`, timeoutMs, async () => {
    const { counts } = await probe(frame);
    if (counts.length !== seen) {
      seen = counts.length;
      since = Date.now();
      return null;
    }
    const last = counts[counts.length - 1];
    return last !== undefined && Date.now() - since >= quietMs ? last.total : null;
  });
}

/** The next download from `page` or from a tab it opens, with a timeout. */
export function nextDownload(ctx: BrowserContext, page: Page, timeoutMs: number): Promise<Download> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no download within ${timeoutMs} ms`)), timeoutMs);
    const done = (d: Download): void => {
      clearTimeout(timer);
      resolve(d);
    };
    page.once("download", done);
    ctx.once("page", (p) => p.once("download", done));
  });
}
