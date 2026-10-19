// Citadel Docs, ours: the shipped editor (DocsCanvas: ribbons, page view, status bar) on the UI's
// bench harness page (citadel-workspace-ui scripts/lib/bench-harness), built from the pinned UI
// commit and served cross-origin isolated by ours/run.sh. The open goes through the page's file
// input; the page count is the status bar's "Page N of M"; the editor paints in a worker, so the
// page reports its paints and each open and keystroke stops at the later of the protocol's frame
// and the frame after the paint (PaintStops). Ours has no Find: the caret goes to the paragraph by
// the editor's own paragraph moves, checked by its own plain-text export. Export is the editor's
// "Download as…" > PDF in its document bar, the editor's own export.

import { readFile } from "node:fs/promises";
import { env } from "node:process";
import type { Frame, Page } from "playwright";
import type { Adapter, KeystrokeTarget, Product } from "../types.js";
import { nextDownload } from "../wait.js";

const UI_TIMEOUT_MS = 60_000;
/** Typed after the caret is placed and exported as text: it must follow the phrase exactly once. */
const MARKER = "Q";

interface BenchWindow {
  __cdocBench?: { painted: number[]; firstPaintEpochMs: number | null };
  __cdocProbe?: { starts: number[] };
  __cdocScrollStep?: () => boolean;
  __cdocScrollTop?: () => void;
}

function required(name: string): string {
  const v = env[name];
  if (v === undefined || v === "") throw new Error(`${name} is not set (ours/run.sh sets it)`);
  return v;
}

const id = (testid: string): string => `[data-testid="${testid}"]`;

/** The editor's keymap (keymap.ts): the paragraph move is Alt on a Mac, Ctrl elsewhere. */
async function paragraphKey(page: Page): Promise<string> {
  const ua = await page.evaluate(() => navigator.userAgent);
  return /Mac|iPhone|iPad|iPod/.test(ua) ? "Alt+ArrowDown" : "Control+ArrowDown";
}

/** "Download as…" > Plain text: the live document's text, as the editor exports it. */
async function exportedText(page: Page): Promise<string> {
  await page.locator(id("docs-download-as")).click();
  const download = nextDownload(page.context(), page, UI_TIMEOUT_MS);
  await page.locator(id("docs-download-as-txt")).click();
  const path = await (await download).path();
  if (path === null) throw new Error("the text export's download has no file");
  return readFile(path, "utf8");
}

const count = (haystack: string, needle: string): number => haystack.split(needle).length - 1;

function create(): Adapter {
  const url = required("CDOC_HARNESS_URL");
  const version = required("CDOC_VERSION");
  return {
    probe: {
      frameUrl: url,
      pageCount: { kind: "label", selector: id("docs-page-status"), pattern: "(\\d+)\\D+(\\d+)" },
    },
    paint: {
      kind: "instrumented",
      opened: (frame: Frame) => frame.evaluate(() => (window as unknown as BenchWindow).__cdocBench?.firstPaintEpochMs ?? null),
      keystroke: (frame: Frame) =>
        frame.evaluate(() => {
          const bench = (window as unknown as BenchWindow).__cdocBench;
          if (bench === undefined) throw new Error("the harness page has no __cdocBench");
          const painted = bench.painted.splice(0);
          if (painted.length > 1) throw new Error(`one keystroke painted ${painted.length} times`);
          return painted[0] ?? null;
        }),
    },
    prepare: async () => undefined,
    version: async () => version,
    async open(page: Page, docx: string) {
      await page.goto(url, { waitUntil: "load" });
      await page.locator(id("bench-open")).setInputFiles(docx);
      const starts = await page.evaluate(() => (window as unknown as BenchWindow).__cdocProbe?.starts ?? []);
      const start = starts[0];
      if (starts.length !== 1 || start === undefined) throw new Error(`the file input stamped ${starts.length} starts`);
      return { page, startEpochMs: start };
    },
    async placeCaret(page: Page, frame: Frame, { phrase, paragraph }: KeystrokeTarget) {
      await page.locator(id("editor-input")).focus();
      await page.keyboard.press("Control+Home");
      const next = await paragraphKey(page);
      // To the start of the paragraph after the target, then one step left: the target's end.
      for (let k = 0; k <= paragraph; k += 1) await page.keyboard.press(next);
      await page.keyboard.press("ArrowLeft");
      await page.keyboard.type(MARKER);
      const text = (await exportedText(page)).toLowerCase();
      // The menu took focus; the editor's textarea gets it back, its caret where it was.
      await page.locator(id("editor-input")).focus();
      await page.keyboard.press("Backspace");
      const found = count(text, `${phrase}${MARKER}`.toLowerCase());
      if (found !== 1) {
        throw new Error(`the caret is not at the end of "${phrase}": "${phrase}${MARKER}" occurs ${found} times in the export`);
      }
      if ((await exportedText(page)).toLowerCase().includes(`${phrase}${MARKER}`.toLowerCase())) {
        throw new Error("the marker typed to check the caret was not deleted");
      }
      await page.locator(id("editor-input")).focus();
      await frame.evaluate(() => (window as unknown as BenchWindow).__cdocBench?.painted.splice(0));
    },
    async installScroll(frame: Frame) {
      await frame.evaluate((selector: string) => {
        const view = document.querySelector(selector);
        if (!(view instanceof HTMLElement)) throw new Error(`no ${selector}`);
        const w = window as unknown as BenchWindow;
        // The page view is the element that scrolls (overflow: auto), not the window.
        w.__cdocScrollStep = () => {
          view.scrollBy(0, view.clientHeight);
          return view.scrollTop + view.clientHeight >= view.scrollHeight - 1;
        };
        w.__cdocScrollTop = () => {
          view.scrollTop = 0;
        };
      }, id("page-view"));
    },
    async openExport(page: Page) {
      const pdf = page.locator(id("docs-download-as-pdf"));
      if (await pdf.isVisible()) return;
      await page.locator(id("docs-download-as")).click();
      await pdf.waitFor({ state: "visible", timeout: UI_TIMEOUT_MS });
    },
    async exportCommand(page: Page) {
      await page.locator(id("docs-download-as-pdf")).click();
    },
  };
}

export const ours: Product = {
  id: "ours",
  tool: "Citadel Docs",
  kind: "ours",
  create,
};
