// The shapes the browser driver reads and writes. `RawRun` mirrors cdoc-bench's
// `schema::RawRun` (the Rust side is the authority: `cdoc-bench summarize --cells browser`
// refuses a file that strays from it), and `BrowserPlan` is what `cdoc-bench browser-plan` writes.

import type { Frame, Page } from "playwright";

export interface Iterations {
  readonly warmup: number;
  readonly iterations: number;
}

/** `cdoc-bench browser-plan`: iterations by cell, keystroke phrase by page count (`100p`). */
export interface BrowserPlan {
  readonly cells: Readonly<Record<string, Iterations>>;
  readonly phrases: Readonly<Record<string, string>>;
}

export type Unmeasured =
  | { readonly status: "not measured"; readonly reason: string }
  | { readonly status: "not comparable"; readonly reason: string };

export interface FrameCount {
  readonly over: number;
  readonly of: number;
}

export interface Provenance {
  readonly kind: "recorded";
  readonly runner: string;
  readonly browser: string;
  readonly commit: string;
  readonly date: string;
  readonly run_url: string;
}

export interface RawRun {
  readonly tool: string;
  readonly version: string;
  readonly samples: Record<string, number[]>;
  readonly bytes: Record<string, number[]>;
  readonly frames: Record<string, FrameCount>;
  readonly unmeasured: Record<string, Unmeasured>;
  readonly provenance: Provenance;
}

/** How the product shows how many pages it has laid out. */
export type PageCount =
  /** A label such as "Page 3 of 100"; `pattern` captures the current page, then the total. */
  | { readonly kind: "label"; readonly selector: string; readonly pattern: string }
  /** One element per laid-out page. */
  | { readonly kind: "elements"; readonly selector: string };

/** What the in-page probe (probe.ts) needs to know about a product. */
export interface ProbeConfig {
  /** A substring of the URL of the frame that holds the document and takes the keystrokes. */
  readonly frameUrl: string;
  readonly pageCount: PageCount;
}

/** The environment variables holding a product's test account, by name. */
export interface SecretNames {
  readonly email: string;
  readonly password: string;
}

export interface Credentials {
  readonly email: string;
  readonly password: string;
}

/**
 * Everything product-specific; the protocol (protocol.ts) is the same for every product. An
 * adapter only drives the product's own UI: where the import starts, where the page count is,
 * how Find puts the caret on a phrase, how one viewport is scrolled, how PDF export is invoked.
 */
export interface Adapter {
  readonly probe: ProbeConfig;
  /** Once per fresh profile, untimed: sign in when the product needs it. */
  prepare(page: Page): Promise<void>;
  /** The product's version as the product reports it. */
  version(page: Page): Promise<string>;
  /**
   * Starts the product's own import of `docx` (an absolute path) and resolves with the page the
   * document opens in and the start, epoch milliseconds taken in the browser
   * (`performance.timeOrigin + performance.now()`).
   */
  open(page: Page, docx: string): Promise<{ readonly page: Page; readonly startEpochMs: number }>;
  /** Puts the caret at the end of `phrase` with the product's Find, untimed. */
  placeCaret(page: Page, frame: Frame, phrase: string): Promise<void>;
  /**
   * Defines `globalThis.__cdocScrollStep` in `frame`: scrolls the document one viewport and
   * returns true once the end is in view; and `globalThis.__cdocScrollTop`, back to page 1.
   */
  installScroll(frame: Frame): Promise<void>;
  /** Everything up to the export command (menus opened), untimed. */
  openExport(page: Page, frame: Frame): Promise<void>;
  /** The export command itself; the download that follows is timed. */
  exportCommand(page: Page, frame: Frame): Promise<void>;
}

/** A product the driver can run. */
export interface Product {
  /** The plan's tool key (`onlyoffice/docx_open/100p`) and the result file's name. */
  readonly id: string;
  /** The name the report shows. */
  readonly tool: string;
  /** The test account it needs, or null when it runs without one. */
  readonly secrets: SecretNames | null;
  create(credentials: Credentials | null): Adapter;
}
