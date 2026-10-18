// The in-page probe, installed in every frame before any of the product's scripts run
// (`BrowserContext.addInitScript`). It is serialised with `Function.prototype.toString`, so it
// must not reference anything outside its own body. It takes the protocol's clocks in the page:
//   - every file input's `change` (the open's start, where the product imports through one);
//   - in the document frame, each change of the page count, stamped at the first animation
//     frame after it (the open's stop);
//   - in the document frame, each `keydown`, from `event.timeStamp` to the second
//     `requestAnimationFrame` callback after it (keystroke to paint).
// Epoch times (`performance.timeOrigin + performance.now()`) let a start in one frame or tab be
// compared with a stop in another.

import type { ProbeConfig } from "./types.js";

export interface PageCountStamp {
  readonly current: number | null;
  readonly total: number;
  readonly atEpochMs: number;
}

export interface ProbeState {
  readonly starts: number[];
  readonly counts: PageCountStamp[];
  readonly keys: number[];
  /** Page-count stamping is switched off once the open is measured, so it adds no work later. */
  watching: boolean;
  readonly isDocumentFrame: boolean;
}

export interface ProbeWindow {
  __cdocProbe?: ProbeState;
}

export function installProbe(cfg: ProbeConfig): void {
  const epoch = (): number => performance.timeOrigin + performance.now();
  const isDocumentFrame = location.href.includes(cfg.frameUrl);
  const state: ProbeState = { starts: [], counts: [], keys: [], watching: true, isDocumentFrame };
  (window as unknown as ProbeWindow).__cdocProbe = state;
  document.addEventListener(
    "change",
    (e: Event) => {
      if (e.target instanceof HTMLInputElement && e.target.type === "file") {
        state.starts.push(epoch());
      }
    },
    true,
  );
  if (!isDocumentFrame) return;
  window.addEventListener(
    "keydown",
    (e: KeyboardEvent) => {
      const start = e.timeStamp;
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          state.keys.push(performance.now() - start);
        }),
      );
    },
    true,
  );
  let last = "";
  const read = (): void => {
    if (!state.watching) return;
    let current: number | null = null;
    let total: number | null = null;
    if (cfg.pageCount.kind === "elements") {
      total = document.querySelectorAll(cfg.pageCount.selector).length;
    } else {
      const text = document.querySelector(cfg.pageCount.selector)?.textContent ?? "";
      const m = new RegExp(cfg.pageCount.pattern).exec(text);
      if (m !== null && m[1] !== undefined && m[2] !== undefined) {
        current = Number(m[1]);
        total = Number(m[2]);
      }
    }
    if (total === null || total === 0) return;
    const key = `${current ?? "-"}/${total}`;
    if (key === last) return;
    last = key;
    const seen = total;
    requestAnimationFrame(() => {
      state.counts.push({ current, total: seen, atEpochMs: epoch() });
    });
  };
  new MutationObserver(read).observe(document, {
    subtree: true,
    childList: true,
    characterData: true,
  });
}
