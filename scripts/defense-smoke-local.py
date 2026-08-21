#!/usr/bin/env python3
"""本机 Edge 防护验收：驱动 WAFtester（可选 gotestwaf/blazehttp），按类型汇总复盘。

仅允许 loopback。复用 scripts/lib/waf_bench_expert.py 的加载与 Edge 探针逻辑。
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_LIB = Path(__file__).resolve().parent / "lib"
if str(_LIB) not in sys.path:
    sys.path.insert(0, str(_LIB))

from waf_bench_expert import (  # noqa: E402
    Case,
    DATASET_LABELS,
    edge_one,
    ensure_dataset,
    load_cases,
    parse_datasets,
)


LOOPBACK_HOSTS = {"127.0.0.1", "localhost", "::1", "[::1]"}


@dataclass
class CatStat:
    category: str
    total: int = 0
    black: int = 0
    white: int = 0
    blocked_ok: int = 0
    missed: int = 0
    passed_ok: int = 0
    false_positive: int = 0
    errors: int = 0
    fn_samples: list[str] = field(default_factory=list)
    fp_samples: list[str] = field(default_factory=list)

    @property
    def detection_rate(self) -> float:
        return self.blocked_ok / self.black if self.black else 1.0

    @property
    def fp_rate(self) -> float:
        return self.false_positive / self.white if self.white else 0.0


def assert_loopback(url: str, label: str) -> None:
    host = (urlparse(url).hostname or "").lower()
    if host not in LOOPBACK_HOSTS:
        raise SystemExit(f"{label} 拒绝非本机目标: {url}（仅 127.0.0.1 / localhost / ::1）")


def category_of(case: Case, dataset: str) -> str:
    """从用例 id / 路径提炼类型名（对齐 waftester payloads/**/*.json）。"""
    cid = case.id.replace("\\", "/")
    # payloads/sqli-advanced.json/<id>
    # payloads/community/ai/prompt-injection.json/<id>
    m = re.search(r"payloads/(.+?)\.json(?:/|$)", cid)
    if m:
        return m.group(1)
    m = re.search(r"testcases/(?:[^/]+/)*([^/#]+)", cid)
    if m:
        name = m.group(1)
        name = re.sub(r"\.(ya?ml|black|white)$", "", name, flags=re.I)
        name = re.sub(r"[-_]\d+$", "", name)
        return name or dataset
    m = re.search(r"/([^/]+)\.(black|white)$", cid, re.I)
    if m:
        return m.group(1)
    parts = [p for p in cid.split("/") if p and p != "payloads"]
    if parts:
        return Path(parts[0]).stem
    return dataset


def probe_http(url: str, timeout: float) -> int:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as resp:
            return int(resp.getcode())
    except urllib.error.HTTPError as e:
        return int(e.code)
    except Exception:
        return 0


def run_dataset(
    *,
    dataset: str,
    cases: list[Case],
    edge: str,
    edge_host: str,
    concurrency: int,
    timeout: float,
) -> tuple[dict[str, CatStat], dict[str, Any], list[tuple[Case, str, bool, str]]]:
    by_cat: dict[str, CatStat] = {}
    results: list[tuple[Case, str, bool, str]] = []
    t0 = time.time()

    def one(c: Case) -> tuple[Case, str, bool, str]:
        return edge_one(edge, edge_host, c, timeout)

    with ThreadPoolExecutor(max_workers=max(1, concurrency)) as pool:
        futs = [pool.submit(one, c) for c in cases]
        done = 0
        for fut in as_completed(futs):
            results.append(fut.result())
            done += 1
            if done % 200 == 0 or done == len(cases):
                print(f"  [{dataset}] progress {done}/{len(cases)}", flush=True)

    for case, actual, _ok, detail in results:
        cat = category_of(case, dataset)
        st = by_cat.get(cat)
        if st is None:
            st = CatStat(category=cat)
            by_cat[cat] = st
        st.total += 1
        if actual == "error":
            st.errors += 1
            continue
        if case.kind == "black":
            st.black += 1
            if actual == "block":
                st.blocked_ok += 1
            else:
                st.missed += 1
                if len(st.fn_samples) < 3:
                    st.fn_samples.append(f"{case.id[:120]} ({detail})")
        else:
            st.white += 1
            if actual == "pass":
                st.passed_ok += 1
            else:
                st.false_positive += 1
                if len(st.fp_samples) < 3:
                    st.fp_samples.append(f"{case.id[:120]} ({detail})")

    black = [r for r in results if r[0].kind == "black"]
    white = [r for r in results if r[0].kind == "white"]
    blocked_ok = sum(1 for r in black if r[1] == "block")
    missed = sum(1 for r in black if r[1] != "block")
    passed_ok = sum(1 for r in white if r[1] == "pass")
    fp = sum(1 for r in white if r[1] == "block")
    errors = sum(1 for r in results if r[1] == "error")
    summary = {
        "dataset": dataset,
        "label": DATASET_LABELS.get(dataset, dataset),
        "total": len(cases),
        "black": len(black),
        "white": len(white),
        "blocked_ok": blocked_ok,
        "missed": missed,
        "passed_ok": passed_ok,
        "false_positive": fp,
        "errors": errors,
        "detection_rate": (blocked_ok / len(black)) if black else 1.0,
        "false_positive_rate": (fp / len(white)) if white else 0.0,
        "elapsed_s": time.time() - t0,
        "categories": len(by_cat),
    }
    return by_cat, summary, results


def print_category_table(dataset: str, by_cat: dict[str, CatStat]) -> None:
    print(f"\n[{DATASET_LABELS.get(dataset, dataset)}] 按类型汇总", flush=True)
    header = (
        f"{'类型':40} {'用例':>6} {'黑':>5} {'白':>5} "
        f"{'检出%':>8} {'误报%':>8} {'FN':>5} {'FP':>4} {'err':>4}"
    )
    print(header, flush=True)
    print("-" * 100, flush=True)
    for cat in sorted(by_cat.keys(), key=str.lower):
        st = by_cat[cat]
        print(
            f"{cat[:40]:40} {st.total:6d} {st.black:5d} {st.white:5d} "
            f"{st.detection_rate * 100:8.1f} {st.fp_rate * 100:8.1f} "
            f"{st.missed:5d} {st.false_positive:4d} {st.errors:4d}",
            flush=True,
        )
    print("-" * 100, flush=True)


def write_markdown(
    path: Path,
    *,
    edge: str,
    edge_host: str,
    started: str,
    finished: str,
    dataset_rows: list[dict[str, Any]],
    category_rows: dict[str, list[dict[str, Any]]],
    min_detection: float,
    max_fp: float,
    gate_ok: bool,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines: list[str] = [
        "# Nerve WAF Edge 本机防护验收（WAFtester 口径）",
        "",
        f"- 开始: {started}",
        f"- 结束: {finished}",
        f"- Edge: `{edge}`",
        f"- Host: `{edge_host}`",
        f"- 门禁: 检出 ≥ {min_detection * 100:.0f}% ，误报 ≤ {max_fp * 100:.0f}%",
        f"- 总体: **{'PASS' if gate_ok else 'FAIL'}**",
        "",
        "## 数据集汇总",
        "",
        "| 数据集 | 用例 | 类型数 | 检出% | 误报% | FN | FP | err | 耗时s |",
        "|--------|------|--------|-------|-------|----|----|-----|-------|",
    ]
    for s in dataset_rows:
        lines.append(
            f"| {s['label']} | {s['total']} | {s['categories']} | "
            f"{s['detection_rate'] * 100:.2f} | {s['false_positive_rate'] * 100:.2f} | "
            f"{s['missed']} | {s['false_positive']} | {s['errors']} | {s['elapsed_s']:.1f} |"
        )
    lines += ["", "## 分类型明细", ""]
    for ds, rows in category_rows.items():
        label = DATASET_LABELS.get(ds, ds)
        lines += [
            f"### {label}",
            "",
            "| 类型 | 用例 | 黑样本 | 白样本 | 检出% | 误报% | FN | FP | err |",
            "|------|------|--------|--------|-------|-------|----|----|-----|",
        ]
        for r in rows:
            lines.append(
                f"| {r['category']} | {r['total']} | {r['black']} | {r['white']} | "
                f"{r['detection_rate'] * 100:.1f} | {r['fp_rate'] * 100:.1f} | "
                f"{r['missed']} | {r['false_positive']} | {r['errors']} |"
            )
        fn_any = [x for r in rows for x in r.get("fn_samples") or []]
        fp_any = [x for r in rows for x in r.get("fp_samples") or []]
        if fn_any:
            lines += ["", "漏拦样例:", ""]
            for s in fn_any[:15]:
                lines.append(f"- `{s}`")
        if fp_any:
            lines += ["", "误拦样例:", ""]
            for s in fp_any[:15]:
                lines.append(f"- `{s}`")
        lines.append("")
    lines += [
        "## 后续动作",
        "",
        "- [ ] FAIL 类型：对照 Console 攻击日志 / 规则包是否加载 / Host 是否同步策略",
        "- [ ] 高 FN：补规则或调 Expert 档位后重跑本脚本",
        "- [ ] 高 FP：查白样本路径，调阈值或加白",
        "- [ ] 全量多数据集：`DATASETS=gotestwaf,waftester,blazehttp`",
        "",
    ]
    path.write_text("\n".join(lines), encoding="utf-8")
    print(f"复盘 Markdown: {path}", flush=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="本机 Edge 防护验收（WAFtester 全类型）")
    parser.add_argument("--edge", default=os.environ.get("EDGE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--edge-host", default=os.environ.get("EDGE_HOST", "test.local"))
    parser.add_argument(
        "--datasets",
        default=os.environ.get("DATASETS", "waftester"),
        help="默认仅 waftester；可 gotestwaf,waftester,blazehttp",
    )
    parser.add_argument("--limit", type=int, default=int(os.environ.get("LIMIT", "0")))
    parser.add_argument("--concurrency", type=int, default=int(os.environ.get("CONCURRENCY", "20")))
    parser.add_argument("--timeout", type=float, default=float(os.environ.get("TIMEOUT", "30")))
    parser.add_argument("--min-detection", type=float, default=float(os.environ.get("MIN_DETECTION", "0.80")))
    parser.add_argument("--max-fp", type=float, default=float(os.environ.get("MAX_FP", "0.10")))
    parser.add_argument("--report-json", default=os.environ.get("REPORT_JSON", ""))
    parser.add_argument("--report-md", default=os.environ.get("REPORT_MD", ""))
    args = parser.parse_args(argv)

    assert_loopback(args.edge, "EDGE")

    script_dir = Path(__file__).resolve().parent
    waf_root = script_dir.parent
    # Nerve-waf → Nerve-waf → Nerve-antibody
    native_default = waf_root.parent / "Nerve-antibody"
    if not native_default.is_dir():
        native_default = waf_root.parent / "Nerve-waf-native"
    native_root = Path(os.environ.get("NATIVE_ROOT", native_default))

    health = f"{args.edge.rstrip('/')}/health"
    code = probe_http(health, 5)
    if code != 200:
        print(f"Edge 未就绪: {health} HTTP {code or '000'}", file=sys.stderr)
        print("请先: bash scripts/start-waf-edge-stack.sh --no-console", file=sys.stderr)
        return 1

    datasets = parse_datasets(args.datasets)
    if not datasets:
        print("未指定数据集", file=sys.stderr)
        return 2

    started = time.strftime("%Y-%m-%dT%H:%M:%S")
    print("=" * 60, flush=True)
    print(" Nerve WAF Edge 本机防护验收（数据集驱动）", flush=True)
    print(f" EDGE={args.edge}  Host={args.edge_host}", flush=True)
    print(f" DATASETS={','.join(datasets)}  LIMIT={args.limit or 'ALL'}", flush=True)
    print(f" NATIVE_ROOT={native_root}", flush=True)
    print("=" * 60, flush=True)

    all_cat_rows: dict[str, list[dict[str, Any]]] = {}
    dataset_summaries: list[dict[str, Any]] = []
    gate_ok = True

    for ds in datasets:
        print(f"\n==> {DATASET_LABELS.get(ds, ds)}", flush=True)
        try:
            ensure_dataset(ds, native_root)
            cases = load_cases(ds, native_root, args.limit)
        except Exception as e:
            print(f"[{ds}] 加载失败: {e}", file=sys.stderr)
            gate_ok = False
            dataset_summaries.append(
                {
                    "dataset": ds,
                    "label": DATASET_LABELS.get(ds, ds),
                    "total": 0,
                    "categories": 0,
                    "detection_rate": 0.0,
                    "false_positive_rate": 1.0,
                    "missed": 0,
                    "false_positive": 0,
                    "errors": 1,
                    "elapsed_s": 0.0,
                    "error": str(e),
                }
            )
            continue

        # 预告类型覆盖
        cats = sorted({category_of(c, ds) for c in cases}, key=str.lower)
        print(f"[{ds}] cases={len(cases)} categories={len(cats)}", flush=True)
        print(f"[{ds}] types: {', '.join(cats)}", flush=True)

        by_cat, summary, _results = run_dataset(
            dataset=ds,
            cases=cases,
            edge=args.edge,
            edge_host=args.edge_host,
            concurrency=args.concurrency,
            timeout=args.timeout,
        )
        print_category_table(ds, by_cat)
        print(
            f"[{ds}] 总计 检出={summary['detection_rate'] * 100:.2f}%  "
            f"误报={summary['false_positive_rate'] * 100:.2f}%  "
            f"FN={summary['missed']} FP={summary['false_positive']} err={summary['errors']}  "
            f"{summary['elapsed_s']:.1f}s",
            flush=True,
        )

        ds_gate = (
            summary["detection_rate"] >= args.min_detection
            and summary["false_positive_rate"] <= args.max_fp
            and summary["errors"] == 0
        )
        if not ds_gate:
            gate_ok = False
            print(f"[{ds}] 门禁 FAIL", flush=True)
        else:
            print(f"[{ds}] 门禁 PASS", flush=True)

        dataset_summaries.append(summary)
        cat_rows = []
        for cat in sorted(by_cat.keys(), key=str.lower):
            st = by_cat[cat]
            cat_rows.append(
                {
                    "category": st.category,
                    "total": st.total,
                    "black": st.black,
                    "white": st.white,
                    "blocked_ok": st.blocked_ok,
                    "missed": st.missed,
                    "passed_ok": st.passed_ok,
                    "false_positive": st.false_positive,
                    "errors": st.errors,
                    "detection_rate": st.detection_rate,
                    "fp_rate": st.fp_rate,
                    "fn_samples": st.fn_samples,
                    "fp_samples": st.fp_samples,
                }
            )
        all_cat_rows[ds] = cat_rows

    finished = time.strftime("%Y-%m-%dT%H:%M:%S")
    report_json = Path(
        args.report_json
        or (waf_root / "logs" / f"defense-smoke-waftester-{time.strftime('%Y%m%d-%H%M%S')}.json")
    )
    report_md = Path(
        args.report_md
        or (waf_root / "logs" / f"defense-smoke-waftester-{time.strftime('%Y%m%d-%H%M%S')}.md")
    )
    payload = {
        "edge": args.edge,
        "edge_host": args.edge_host,
        "started": started,
        "finished": finished,
        "min_detection": args.min_detection,
        "max_fp": args.max_fp,
        "gate_ok": gate_ok,
        "datasets": dataset_summaries,
        "by_category": all_cat_rows,
    }
    report_json.parent.mkdir(parents=True, exist_ok=True)
    report_json.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"复盘 JSON: {report_json}", flush=True)

    write_markdown(
        report_md,
        edge=args.edge,
        edge_host=args.edge_host,
        started=started,
        finished=finished,
        dataset_rows=dataset_summaries,
        category_rows=all_cat_rows,
        min_detection=args.min_detection,
        max_fp=args.max_fp,
        gate_ok=gate_ok,
    )

    print("\n" + ("本机防护验收 PASS" if gate_ok else "本机防护验收 FAIL"), flush=True)
    return 0 if gate_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
