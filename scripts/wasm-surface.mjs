// Prints a WebAssembly module's surface, sorted, one per line: `import <module>` for every
// module it imports from and `export <name>` for every export. Node's own parser, no tool.
//   node scripts/wasm-surface.mjs FILE
import { readFileSync } from 'node:fs';

const file = process.argv[2];
if (!file) throw new Error('usage: node scripts/wasm-surface.mjs FILE');
const module = new WebAssembly.Module(readFileSync(file));
const lines = new Set([
  ...WebAssembly.Module.imports(module).map((i) => `import ${i.module}`),
  ...WebAssembly.Module.exports(module).map((e) => `export ${e.name}`),
]);
process.stdout.write([...lines].sort().join('\n') + '\n');
