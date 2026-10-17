"""LibreOffice warm: one running soffice, driven over UNO, opening each `.docx` fixture
(loadComponentFromURL, hidden) and exporting it to PDF (storeToURL, writer_pdf_Export).
Times are integer nanoseconds from time.perf_counter_ns(). Writes raw samples for
`cdoc-bench summarize`.

usage: python3 uno_bench.py PORT VERSION OUT.json CORPUS_DIR PAGES:WARMUP:ITERATIONS ..
"""

import json
import os
import sys
import tempfile
import time

import uno  # python3-uno, the version matching the installed LibreOffice
from com.sun.star.beans import PropertyValue
from com.sun.star.connection import NoConnectException


def prop(name, value):
    p = PropertyValue()
    p.Name = name
    p.Value = value
    return p


def connect(port):
    local = uno.getComponentContext()
    resolver = local.ServiceManager.createInstanceWithContext(
        "com.sun.star.bridge.UnoUrlResolver", local
    )
    url = f"uno:socket,host=127.0.0.1,port={port};urp;StarOffice.ComponentContext"
    deadline = time.monotonic() + 60
    while True:
        try:
            ctx = resolver.resolve(url)
            break
        except NoConnectException:  # soffice still starting; retried until the deadline
            if time.monotonic() > deadline:
                raise
            time.sleep(0.5)
    return ctx.ServiceManager.createInstanceWithContext("com.sun.star.frame.Desktop", ctx)


def once(desktop, src, dst):
    """Returns (open_ns, open_and_export_ns)."""
    start = time.perf_counter_ns()
    doc = desktop.loadComponentFromURL(src, "_blank", 0, (prop("Hidden", True),))
    opened = time.perf_counter_ns()
    if doc is None:
        raise RuntimeError(f"LibreOffice could not open {src}")
    try:
        doc.storeToURL(dst, (prop("FilterName", "writer_pdf_Export"),))
        done = time.perf_counter_ns()
    finally:
        doc.close(True)
    if os.path.getsize(uno.fileUrlToSystemPath(dst)) == 0:
        raise RuntimeError(f"empty PDF from {src}")
    return opened - start, done - start


def main(argv):
    if len(argv) < 6:
        sys.exit(__doc__)
    port, version, out, corpus = argv[1], argv[2], argv[3], argv[4]
    desktop = connect(port)
    samples = {}
    unmeasured = {}
    scratch = tempfile.mkdtemp()
    for run in argv[5:]:
        pages, warmup, iterations = (int(n) for n in run.split(":"))
        if iterations < 1:
            raise ValueError(f"bad run {run}")
        src = uno.systemPathToFileUrl(os.path.abspath(f"{corpus}/{pages}p.docx"))
        dst = uno.systemPathToFileUrl(os.path.join(scratch, f"{pages}p.pdf"))
        for _ in range(warmup):
            once(desktop, src, dst)
        opens, totals = [], []
        for _ in range(iterations):
            o, t = once(desktop, src, dst)
            opens.append(o)
            totals.append(t)
        samples[f"docx_open/{pages}p"] = opens
        samples[f"docx_to_pdf/{pages}p"] = totals
        unmeasured[f"pdf_export/{pages}p"] = {
            "status": "not comparable",
            "reason": "LibreOffice's export lays the document out; compare docx_to_pdf",
        }
    raw = {"tool": "LibreOffice (warm, UNO)", "version": version,
           "samples": samples, "unmeasured": unmeasured}
    with open(out, "w", encoding="utf-8") as f:
        json.dump(raw, f, indent=2)


if __name__ == "__main__":
    main(sys.argv)
