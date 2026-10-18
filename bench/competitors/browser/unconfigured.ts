// A product that needs a test account runs only when both of its secrets are set. Without them
// every cell says so, explicitly: never a number, never silently green, never a guess.

import type { BrowserPlan, Credentials, Product, Provenance, RawRun, Unmeasured } from "./types.js";

export type Env = Readonly<Record<string, string | undefined>>;

/** The test account from `env`, or the names of the secrets that are missing or empty. */
export function credentials(product: Product, env: Env): Credentials | null | string[] {
  if (product.secrets === null) return null;
  const { email, password } = product.secrets;
  const missing = [email, password].filter((name) => (env[name] ?? "").trim() === "");
  if (missing.length > 0) return missing;
  return { email: env[email] ?? "", password: env[password] ?? "" };
}

/** Every protocol cell of the plan (and the long-frame count read off the scroll), unmeasured. */
export function unconfigured(
  product: Product,
  plan: BrowserPlan,
  missing: readonly string[],
  provenance: Provenance,
): RawRun {
  const reason =
    `no test account configured: the repository secrets ${missing.join(" and ")} are not set ` +
    "(bench/competitors/README.md, accounts)";
  const cell: Unmeasured = { status: "not measured", reason };
  const unmeasured: Record<string, Unmeasured> = {};
  for (const name of Object.keys(plan.cells)) {
    unmeasured[name] = cell;
    if (name.startsWith("scroll/")) unmeasured[`scroll_long_frames/${name.slice(7)}`] = cell;
  }
  return {
    tool: product.tool,
    version: "not measured",
    samples: {},
    bytes: {},
    frames: {},
    unmeasured,
    provenance,
  };
}
