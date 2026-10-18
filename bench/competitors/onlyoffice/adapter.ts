// ONLYOFFICE Docs (Community, the Document Server image run.sh pins) through its DocsAPI embed.
// The editor renders to canvas inside an iframe, so the open stops at the page-count label
// ("Page N of M", #label-pages) showing the full count; it scrolls its own canvas, not the
// window, through its own wheel handler, until its API says the last page is drawn; Find is its search bar (Ctrl+F);
// PDF export is File > Download as > PDF.

import { basename } from "node:path";
import { env } from "node:process";
import type { Frame, Page } from "playwright";
import type { Adapter, KeystrokeTarget, Product } from "../browser/types.js";

const FRAME = "/web-apps/apps/documenteditor/main/";
/** `Asc.c_oAscFileType.PDF`, the format attribute of the PDF button in Download as. */
const PDF_FORMAT = "513";
const UI_TIMEOUT_MS = 60_000;
/** "Page N of M": the current page, then the total. */
const PAGE_PATTERN = "(\\d+)\\D+(\\d+)";

interface Host {
  DocsAPI?: { DocEditor: { version(): string } };
  cdocOpen?: (file: string, key: string) => number;
}

function required(name: string): string {
  const v = env[name];
  if (v === undefined || v === "") throw new Error(`${name} is not set (run.sh sets it)`);
  return v;
}

function create(): Adapter {
  const hostUrl = required("ONLYOFFICE_HOST_URL");
  const image = required("ONLYOFFICE_IMAGE");
  let opens = 0;
  const loadHost = async (page: Page): Promise<void> => {
    await page.goto(hostUrl, { waitUntil: "load" });
    await page.waitForFunction(() => (window as unknown as Host).DocsAPI !== undefined);
  };
  return {
    paint: { kind: "protocol" },
    probe: {
      frameUrl: FRAME,
      pageCount: { kind: "label", selector: "#label-pages", pattern: PAGE_PATTERN },
    },
    prepare: async () => undefined,
    async version(page: Page) {
      await loadHost(page);
      const v = await page.evaluate(() => (window as unknown as Host).DocsAPI?.DocEditor.version());
      if (v === undefined || v === "") throw new Error("DocsAPI reported no version");
      return `${v} (${image})`;
    },
    async open(page: Page, docx: string) {
      await loadHost(page);
      opens += 1;
      // A fresh key per open: the Document Server converts the file again instead of serving
      // its cached conversion.
      const key = `${basename(docx, ".docx")}-${Date.now()}-${opens}`;
      const startEpochMs = await page.evaluate(
        ([file, k]) => {
          const open = (window as unknown as Host).cdocOpen;
          if (open === undefined) throw new Error("host.html did not define cdocOpen");
          return open(file, k);
        },
        [basename(docx), key] as const,
      );
      return { page, startEpochMs };
    },
    async placeCaret(page: Page, frame: Frame, { phrase }: KeystrokeTarget) {
      await frame.locator("#id_main_view").click();
      await page.keyboard.press("Control+f");
      const input = frame.locator("#search-bar-text");
      await input.waitFor({ state: "visible", timeout: UI_TIMEOUT_MS });
      await input.fill(phrase);
      await input.press("Enter");
      await frame.locator("#search-bar-results", { hasText: /^\s*1\/1\s*$/ }).waitFor({ timeout: UI_TIMEOUT_MS });
      await page.keyboard.press("Escape");
      // Until the bar has closed, the arrow would go to its input and leave the match selected
      // (the first keystroke would then replace it).
      await input.waitFor({ state: "hidden", timeout: UI_TIMEOUT_MS });
      await page.keyboard.press("ArrowRight");
    },
    async installScroll(frame: Frame) {
      await frame.evaluate(() => {
        interface Api {
          getCountPages(): number;
          GetCurrentVisiblePages(): number[];
        }
        const w = window as unknown as {
          editor?: Partial<Api>;
          __cdocScrollStep?: () => boolean;
          __cdocScrollTop?: () => void;
        };
        const api = w.editor;
        const view = document.getElementById("id_main_view");
        if (
          view === null ||
          typeof api?.getCountPages !== "function" ||
          typeof api.GetCurrentVisiblePages !== "function"
        ) {
          throw new Error("no #id_main_view, or the editor API lacks getCountPages/GetCurrentVisiblePages");
        }
        const pages = api as Api;
        // The editor's own wheel handler (#id_main's onmousewheel) scrolls by
        // -45 * wheelDeltaY / 120 pixels, so this wheelDeltaY scrolls by `dy`.
        const wheel = (dy: number): void => {
          const e = new WheelEvent("mousewheel", { bubbles: true, cancelable: true, deltaY: dy });
          const wheelDelta = (-dy * 120) / 45;
          Object.defineProperty(e, "wheelDeltaY", { value: wheelDelta });
          Object.defineProperty(e, "wheelDelta", { value: wheelDelta });
          view.dispatchEvent(e);
        };
        // The page label follows the caret, not the view: the end is the last page drawn.
        w.__cdocScrollStep = () => {
          wheel(view.clientHeight);
          return pages.GetCurrentVisiblePages().includes(pages.getCountPages() - 1);
        };
        w.__cdocScrollTop = () => wheel(-1e9);
      });
    },
    async openExport(_page: Page, frame: Frame) {
      const pdf = frame.locator(`.btn-doc-format[format="${PDF_FORMAT}"]`);
      if (await pdf.isVisible()) return;
      const download = frame.locator("#fm-btn-download");
      if (!(await download.isVisible())) await frame.locator('a[data-tab="file"]').click();
      await download.click();
      await pdf.waitFor({ state: "visible", timeout: UI_TIMEOUT_MS });
    },
    async exportCommand(_page: Page, frame: Frame) {
      await frame.locator(`.btn-doc-format[format="${PDF_FORMAT}"]`).click();
    },
  };
}

export const onlyoffice: Product = {
  id: "onlyoffice",
  tool: "ONLYOFFICE Docs",
  secrets: null,
  kind: "competitor",
  create,
};
