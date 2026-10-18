// The UI's dependencies as the harness build installs them, derived from the pinned UI's own
// package.json (the one source): its runtime dependencies less the parent repository's two local
// packages (`file:` paths to citadel-workspace, which the harness page never reaches; its build
// would fail if it did), and of its dev dependencies only the build tools serve.mjs runs. No
// scripts (its `prepare` sets git hooks). `ui.package-lock.json` pins this set; `npm ci` refuses
// a package.json that has drifted from it, so a new UI pin regenerates the lock (README.md).
//
// usage: node dist/browser/ours/ui-deps.js UI_PACKAGE_JSON OUT_PACKAGE_JSON

import { readFile, writeFile } from "node:fs/promises";
import { argv } from "node:process";

/** The parent's packages, linked by path; absent from a standalone checkout. */
export const PARENT_PACKAGES = ["citadel-internal-service-wasm-client", "citadel-workspace-client-ts"] as const;
/** What serve.mjs and the app's vite.config.ts load at build time. */
export const BUILD_TOOLS = [
  "vite",
  "@vitejs/plugin-react-swc",
  "vite-plugin-pwa",
  "postcss",
  "tailwindcss",
  "autoprefixer",
  "@tailwindcss/typography",
] as const;

type Deps = Record<string, string>;

export interface UiPackage {
  readonly [key: string]: unknown;
  readonly dependencies: Deps;
  readonly devDependencies: Deps;
}

function deps(value: unknown, what: string): Deps {
  if (typeof value !== "object" || value === null) throw new Error(`the UI's package.json has no ${what}`);
  for (const [name, spec] of Object.entries(value)) {
    if (typeof spec !== "string") throw new Error(`${what}.${name} is not a version`);
  }
  return value as Deps;
}

/** The harness build's package.json from the UI's; fails on any drift it cannot account for. */
export function harnessPackage(ui: Readonly<Record<string, unknown>>): Record<string, unknown> {
  const dependencies = { ...deps(ui.dependencies, "dependencies") };
  for (const name of PARENT_PACKAGES) {
    if (!dependencies[name]?.startsWith("file:")) throw new Error(`${name} is no longer a file: dependency of the UI`);
    delete dependencies[name];
  }
  const local = Object.entries(dependencies).filter(([, spec]) => spec.startsWith("file:"));
  if (local.length > 0) throw new Error(`the UI links more local packages: ${local.map(([n]) => n).join(", ")}`);
  const dev = deps(ui.devDependencies, "devDependencies");
  const devDependencies: Deps = {};
  for (const name of BUILD_TOOLS) {
    const spec = dev[name];
    if (spec === undefined) throw new Error(`the UI has no dev dependency ${name}`);
    devDependencies[name] = spec;
  }
  const { scripts: _scripts, ...rest } = ui;
  return { ...rest, dependencies, devDependencies };
}

async function main(): Promise<void> {
  const [input, output] = argv.slice(2);
  if (input === undefined || output === undefined) throw new Error("usage: ui-deps.js UI_PACKAGE_JSON OUT");
  const ui = JSON.parse(await readFile(input, "utf8")) as Record<string, unknown>;
  await writeFile(output, `${JSON.stringify(harnessPackage(ui), null, 2)}\n`);
}

if (argv[1]?.endsWith("ui-deps.js") === true) {
  main().catch((e: unknown) => {
    console.error(e instanceof Error ? e.message : String(e));
    process.exitCode = 1;
  });
}
