#!/usr/bin/env python3
"""对运行中的 Nerve-waf 跑多数据集 WAF 评测（gotestwaf / blazehttp / waftester / crs-ftw）并输出汇总表。"""
from __future__ import annotations

import sys
from pathlib import Path

_LIB = Path(__file__).resolve().parent / "lib"
if str(_LIB) not in sys.path:
    sys.path.insert(0, str(_LIB))

from waf_bench_expert import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
