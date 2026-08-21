#!/usr/bin/env python3
"""形态 B：gotestwaf / blazehttp / waftester → nerve-waf-edge（真实 HTTP 流量）。"""
from __future__ import annotations

import os
import sys
from pathlib import Path

_LIB = Path(__file__).resolve().parent / "lib"
if str(_LIB) not in sys.path:
    sys.path.insert(0, str(_LIB))

from waf_bench_expert import main  # noqa: E402

if __name__ == "__main__":
    edge = os.environ.get("WAF_EDGE_STACK_EDGE", os.environ.get("NERVE_WAF_EDGE_URL", "http://127.0.0.1:8080"))
    host = os.environ.get("WAF_EDGE_BENCH_HOST", os.environ.get("NERVE_WAF_EDGE_BENCH_HOST", "test.local"))
    argv = list(sys.argv[1:])
    if not any(a.startswith("--edge") for a in argv):
        argv = ["--edge", edge, "--edge-host", host, *argv]
    sys.exit(main(argv))
