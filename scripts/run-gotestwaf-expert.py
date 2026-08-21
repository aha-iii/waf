#!/usr/bin/env python3
"""兼容入口：仅跑 GoTestWAF（等价于 run-waf-bench-expert.py --datasets gotestwaf）。"""
from __future__ import annotations

import sys
from pathlib import Path

_LIB = Path(__file__).resolve().parent / "lib"
if str(_LIB) not in sys.path:
    sys.path.insert(0, str(_LIB))

from waf_bench_expert import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main(single_dataset="gotestwaf"))
