// Google Docs, signed in with the test account in BENCH_GDOCS_EMAIL and BENCH_GDOCS_PASSWORD
// (repository secrets; without both, every cell is "not measured").
//
// NOT YET RUN: no test account exists yet, so every selector below is unverified against the
// live product, and Google may refuse a scripted sign-in outright ("this browser or app may not
// be secure"). The first run with an account will fail loudly at the first wrong step
// (screenshots in the artifact), never measure something else silently.
//
// Open: Docs' own file picker, Upload (a file input: the clock starts at its `change`); Docs
// renders each page as a canvas tile, so the open stops when the page elements reach the full
// count. Scroll is the editor's own scroller; PDF export is File > Download > PDF.

import type { Frame, Page } from "playwright";
import type { Adapter, Credentials, KeystrokeTarget, Product } from "../browser/types.js";

const HOME = "https://docs.google.com/document/u/0/";
const FRAME = "docs.google.com/document/d/";
const UI_TIMEOUT_MS = 60_000;

function create(account: Credentials | null): Adapter {
  if (account === null) throw new Error("Google Docs needs its test account");
  return {
    paint: { kind: "protocol" },
    probe: { frameUrl: FRAME, pageCount: { kind: "elements", selector: ".kix-page-paginated" } },
    async prepare(page: Page) {
      await page.goto("https://accounts.google.com/signin");
      await page.locator('input[type="email"]').fill(account.email);
      await page.getByRole("button", { name: "Next" }).click();
      await page.locator('input[type="password"]').fill(account.password, { timeout: UI_TIMEOUT_MS });
      await page.getByRole("button", { name: "Next" }).click();
      await page.waitForURL(/myaccount\.google\.com|google\.com\/?$/, { timeout: UI_TIMEOUT_MS });
    },
    async version() {
      return "Google Docs (the service exposes no version; dated by provenance)";
    },
    async open(page: Page, docx: string) {
      await page.goto(HOME);
      await page.getByRole("button", { name: /Open file picker/ }).click();
      const picker = page.frameLocator('iframe[src*="picker"]');
      await picker.getByRole("tab", { name: "Upload" }).click({ timeout: UI_TIMEOUT_MS });
      const chooser = page.waitForEvent("filechooser", { timeout: UI_TIMEOUT_MS });
      await picker.getByRole("button", { name: /Browse/ }).click();
      await (await chooser).setFiles(docx);
      let startEpochMs: number | undefined;
      for (const frame of page.frames()) {
        const starts = await frame
          .evaluate(() => (window as unknown as { __cdocProbe?: { starts: number[] } }).__cdocProbe?.starts ?? [])
          .catch(() => []);
        startEpochMs ??= starts[0];
      }
      if (startEpochMs === undefined) throw new Error("the picker's upload input fired no change");
      return { page, startEpochMs };
    },
    async placeCaret(page: Page, frame: Frame, { phrase }: KeystrokeTarget) {
      await page.keyboard.press("Control+f");
      const input = frame.locator(".docs-findinput-input");
      await input.fill(phrase, { timeout: UI_TIMEOUT_MS });
      await input.press("Enter");
      await page.keyboard.press("Escape");
      await page.keyboard.press("ArrowRight");
    },
    async installScroll(frame: Frame) {
      await frame.evaluate(() => {
        const view = document.querySelector(".kix-appview-editor");
        if (view === null) throw new Error("Docs' .kix-appview-editor scroller not found");
        const w = window as unknown as { __cdocScrollStep?: () => boolean; __cdocScrollTop?: () => void };
        w.__cdocScrollStep = () => {
          view.scrollBy(0, view.clientHeight);
          return view.scrollTop + view.clientHeight >= view.scrollHeight - 1;
        };
        w.__cdocScrollTop = () => view.scrollTo(0, 0);
      });
    },
    async openExport(_page: Page, frame: Frame) {
      await frame.locator("#docs-file-menu").click();
      await frame.getByRole("menuitem", { name: /^Download/ }).hover();
    },
    async exportCommand(_page: Page, frame: Frame) {
      await frame.getByRole("menuitem", { name: /PDF Document/ }).click();
    },
  };
}

export const googleDocs: Product = {
  id: "google-docs",
  tool: "Google Docs",
  secrets: { email: "BENCH_GDOCS_EMAIL", password: "BENCH_GDOCS_PASSWORD" },
  kind: "competitor",
  create,
};
