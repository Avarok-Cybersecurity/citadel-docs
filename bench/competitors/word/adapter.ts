// Microsoft Word for the web, signed in with the test account in BENCH_WORD_EMAIL and
// BENCH_WORD_PASSWORD (repository secrets; without both, every cell is "not measured").
//
// NOT YET RUN: no test account exists yet, so every selector below is unverified against the
// live product. The first run with an account will fail loudly at the first selector that is
// wrong (screenshots in the artifact), never measure something else silently.
//
// Open: the start page's Upload (a file input: the clock starts at its `change`), the document
// opening in the Word editor's frame; stop at its status-bar label "Page N of M".

import type { Frame, Page } from "playwright";
import type { Adapter, Credentials, KeystrokeTarget, Product } from "../browser/types.js";

const START = "https://www.office.com/launch/word";
const FRAME = "officeapps.live.com/we/wordeditorframe";
const UI_TIMEOUT_MS = 60_000;

function create(account: Credentials | null): Adapter {
  if (account === null) throw new Error("Word for the web needs its test account");
  return {
    paint: { kind: "protocol" },
    probe: {
      frameUrl: FRAME,
      pageCount: { kind: "label", selector: '[id*="PageNumber"]', pattern: "(\\d+)\\D+(\\d+)" },
    },
    async prepare(page: Page) {
      await page.goto(START);
      await page.locator('input[type="email"]').fill(account.email);
      await page.locator('input[type="submit"]').click();
      await page.locator('input[type="password"]').fill(account.password);
      await page.locator('input[type="submit"]').click();
      const staySignedIn = page.getByRole("button", { name: /^No$/ });
      await staySignedIn.click({ timeout: UI_TIMEOUT_MS });
      await page.waitForURL(/office\.com|microsoft365\.com/, { timeout: UI_TIMEOUT_MS });
    },
    async version() {
      return "Word for the web (the service exposes no version; dated by provenance)";
    },
    async open(page: Page, docx: string) {
      await page.goto(START);
      const opened = page.context().waitForEvent("page", { timeout: UI_TIMEOUT_MS });
      const chooser = page.waitForEvent("filechooser", { timeout: UI_TIMEOUT_MS });
      await page.getByRole("button", { name: /Upload/ }).click();
      await (await chooser).setFiles(docx);
      const tab = await opened;
      const starts = await page.evaluate(
        () => (window as unknown as { __cdocProbe?: { starts: number[] } }).__cdocProbe?.starts ?? [],
      );
      const startEpochMs = starts[0];
      if (startEpochMs === undefined) throw new Error("the upload input fired no change");
      return { page: tab, startEpochMs };
    },
    async placeCaret(page: Page, frame: Frame, { phrase }: KeystrokeTarget) {
      await page.keyboard.press("Control+f");
      const input = frame.getByRole("searchbox");
      await input.fill(phrase, { timeout: UI_TIMEOUT_MS });
      await input.press("Enter");
      await page.keyboard.press("Escape");
      await page.keyboard.press("ArrowRight");
    },
    async installScroll(frame: Frame) {
      await frame.evaluate(() => {
        const view = document.getElementById("WACViewPanel");
        if (view === null) throw new Error("Word's #WACViewPanel scroller not found");
        const w = window as unknown as { __cdocScrollStep?: () => boolean; __cdocScrollTop?: () => void };
        w.__cdocScrollStep = () => {
          view.scrollBy(0, view.clientHeight);
          return view.scrollTop + view.clientHeight >= view.scrollHeight - 1;
        };
        w.__cdocScrollTop = () => view.scrollTo(0, 0);
      });
    },
    async openExport(_page: Page, frame: Frame) {
      await frame.getByRole("tab", { name: "File" }).click();
      await frame.getByRole("menuitem", { name: /Create a Copy|Download/ }).click();
    },
    async exportCommand(_page: Page, frame: Frame) {
      await frame.getByRole("menuitem", { name: /Download as PDF/ }).click();
      await frame.getByRole("button", { name: /^Download$/ }).click({ timeout: UI_TIMEOUT_MS });
    },
  };
}

export const word: Product = {
  id: "word",
  tool: "Microsoft Word (web)",
  secrets: { email: "BENCH_WORD_EMAIL", password: "BENCH_WORD_PASSWORD" },
  kind: "competitor",
  create,
};
