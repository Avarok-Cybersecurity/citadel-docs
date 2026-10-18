// The harness build's dependencies: the UI's own, less the parent's two linked packages and all
// but the build tools of its dev dependencies; any drift that could change what is built fails.

import assert from "node:assert/strict";
import { test } from "node:test";
import { BUILD_TOOLS, harnessPackage } from "./ui-deps.js";

const ui = (): Record<string, unknown> => ({
  name: "citadel-workspaces",
  type: "module",
  scripts: { prepare: "git config core.hooksPath .githooks" },
  dependencies: {
    react: "^18.3.1",
    "citadel-internal-service-wasm-client": "file:../citadel-internal-service/typescript-client",
    "citadel-workspace-client-ts": "file:../citadel-workspace-client-ts",
  },
  devDependencies: { ...Object.fromEntries(BUILD_TOOLS.map((t) => [t, "^1.0.0"])), vitest: "^3.0.7" },
  optionalDependencies: { "@swc/core-linux-x64-gnu": "1.15.8" },
});

test("keeps runtime dependencies and build tools, drops the parent's packages, tests and scripts", () => {
  const out = harnessPackage(ui());
  assert.deepEqual(out.dependencies, { react: "^18.3.1" });
  assert.deepEqual(Object.keys(out.devDependencies as object), [...BUILD_TOOLS]);
  assert.equal(out.scripts, undefined);
  assert.deepEqual(out.optionalDependencies, { "@swc/core-linux-x64-gnu": "1.15.8" });
});

test("a parent package that is no longer linked, a new linked one, or a missing build tool fails", () => {
  const moved = ui();
  (moved.dependencies as Record<string, string>)["citadel-workspace-client-ts"] = "^1.0.0";
  assert.throws(() => harnessPackage(moved), /no longer a file: dependency/);
  const linked = ui();
  (linked.dependencies as Record<string, string>)["other"] = "file:../other";
  assert.throws(() => harnessPackage(linked), /more local packages: other/);
  const tools = ui();
  delete (tools.devDependencies as Record<string, string>)["vite"];
  assert.throws(() => harnessPackage(tools), /no dev dependency vite/);
});
