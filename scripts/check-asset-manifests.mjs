// Checks every file an asset manifest pins: it exists and its sha256 is the one recorded. The
// UI fetches these files and verifies the same hashes, so a file edited without its manifest
// entry (or the other way round) is red here first.
//   node scripts/check-asset-manifests.mjs
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const manifests = [
  ['fonts/manifest.json', (m) => [...m.faces, ...m.testFixtures]],
  ['crates/cdoc-para/patterns/manifest.json', (m) => m.patterns],
];
let failed = 0;
for (const [path, entries] of manifests) {
  const listed = entries(JSON.parse(readFileSync(path, 'utf8')));
  for (const { file, sha256 } of listed) {
    const actual = createHash('sha256').update(readFileSync(join(dirname(path), file))).digest('hex');
    if (actual !== sha256) {
      console.error(`${path}: ${file} is ${actual}, the manifest says ${sha256}`);
      failed += 1;
    }
  }
  console.log(`${path}: ${listed.length} files checked`);
}
process.exit(failed === 0 ? 0 : 1);
