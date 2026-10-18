// The host for ONLYOFFICE's editor: the embed page (host.html), the corpus `.docx` files the
// Document Server downloads, and the edit-session callback (answered `{"error":0}`, as the
// Document Server requires). Runner-only, plain HTTP, no JWT (the container runs with
// JWT_ENABLED=false; see run.sh).
//
// usage: node dist/onlyoffice/server.js --port N --corpus DIR --ds-url URL --doc-base URL
//   --ds-url:   the Document Server as the browser reaches it (http://localhost:8080)
//   --doc-base: this server as the Document Server's container reaches it

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { parseArgs } from "node:util";

const FLAGS = ["port", "corpus", "ds-url", "doc-base"] as const;
const { values } = parseArgs({
  options: Object.fromEntries(FLAGS.map((f) => [f, { type: "string" as const }])),
  strict: true,
});
const arg = (f: (typeof FLAGS)[number]): string => {
  const v = values[f];
  if (typeof v !== "string" || v === "") throw new Error(`missing --${f}`);
  return v;
};
const port = Number.parseInt(arg("port"), 10);
const corpus = arg("corpus");
const template = await readFile(new URL("../../onlyoffice/host.html", import.meta.url), "utf8");
const host = template.replaceAll("__DS_URL__", arg("ds-url")).replaceAll("__DOC_BASE__", arg("doc-base"));
/** Only the corpus' own names are served; nothing else on disk is reachable. */
const DOCX = /^\/corpus\/(\d+p\.docx)$/;

createServer((req, res) => {
  const url = req.url ?? "";
  if (req.method === "GET" && url.split("?")[0] === "/") {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" }).end(host);
    return;
  }
  const docx = DOCX.exec(url);
  if (req.method === "GET" && docx?.[1] !== undefined) {
    readFile(`${corpus}/${docx[1]}`).then(
      (bytes) => res.writeHead(200, { "content-type": "application/octet-stream" }).end(bytes),
      () => res.writeHead(404).end(),
    );
    return;
  }
  if (req.method === "POST" && url === "/callback") {
    let body = "";
    req.on("data", (chunk: Buffer) => (body += chunk.toString("utf8")));
    req.on("end", () => {
      try {
        const status = (JSON.parse(body) as { status?: unknown }).status;
        console.log(`callback: status ${String(status)}`);
        res.writeHead(200, { "content-type": "application/json" }).end('{"error":0}');
      } catch {
        res.writeHead(400).end();
      }
    });
    return;
  }
  res.writeHead(404).end();
}).listen(port, "0.0.0.0", () => console.log(`host on :${port}`));
