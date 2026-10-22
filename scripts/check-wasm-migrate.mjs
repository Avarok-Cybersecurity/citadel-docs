// The shipped artefact migrates a chat document under any non-zero u64 client, passed as the
// BigInt the UI derives (past 2^53, where a JavaScript number would round, up to 2^64 - 1),
// writes exactly that client into the snapshot's state vector, refuses client 0, and renders
// the same document as ProseMirror JSON. Run on the bindgen output by scripts/build-wasm.sh.
//   node scripts/check-wasm-migrate.mjs OUT NAME
import { copyFileSync, readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

const [out, name] = process.argv.slice(2);
if (!out || !name) throw new Error('usage: node scripts/check-wasm-migrate.mjs OUT NAME');
// The glue is an ES module in a directory without a package.json; as .mjs Node reads it so.
const module = join(out, `cdoc_wasm_${name}.check.mjs`);
copyFileSync(join(out, `cdoc_wasm_${name}.js`), module);
const glue = await import(pathToFileURL(module).href);
glue.initSync({ module: readFileSync(join(out, `cdoc_wasm_${name}_bg.wasm`)) });

const doc = JSON.stringify({
  type: 'doc',
  content: [{ type: 'paragraph', content: [{ type: 'text', text: 'shalom שלום 😀' }] }],
});

// The snapshot header is "CDC", version, kind; then the state vector: a count, then
// (client, next clock) pairs, all unsigned LEB128.
function writers(bytes) {
  let at = 5;
  const varint = () => {
    let value = 0n;
    for (let shift = 0n; ; shift += 7n) {
      const byte = bytes[at++];
      value |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return value;
    }
  };
  const clients = [];
  for (let n = varint(); n > 0n; n--) {
    clients.push(varint());
    varint();
  }
  return clients;
}

const fail = (msg) => { console.error(`check-wasm-migrate ${name}: ${msg}`); process.exit(1); };
const clients = [1n, 2n ** 53n + 1n, 2n ** 64n - 1n];
const seen = new Set();
for (const client of clients) {
  const migrated = glue.tiptap_to_crdt_snapshot(doc, client);
  const snapshot = migrated.snapshot();
  const report = JSON.parse(migrated.reportJson());
  migrated.free();
  const got = writers(snapshot);
  if (got.length !== 1 || got[0] !== client) fail(`client ${client} wrote ${got.join(',')}`);
  if (report.problems.length !== 0) fail(`problems ${JSON.stringify(report.problems)}`);
  seen.add(Buffer.from(snapshot).toString('hex'));
}
if (seen.size !== clients.length) fail('two clients wrote the same bytes');
let refusal = 'accepted';
try { glue.tiptap_to_crdt_snapshot(doc, 0n); } catch (e) { refusal = String(e); }
if (!refusal.includes('client 0 is reserved')) fail(`client 0 was not refused as reserved: ${refusal}`);
const pm = JSON.parse(glue.tiptap_to_pm(doc));
if (pm.type !== 'doc' || pm.content[0].type !== 'section') fail('tiptap_to_pm is not a doc of sections');
glue.check_pm(JSON.stringify(pm));
console.log(`check-wasm-migrate ${name}: clients ${clients.join(', ')} OK, client 0 refused, pm OK`);
