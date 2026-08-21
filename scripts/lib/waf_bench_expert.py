"""WAF 评测集 → Nerve-waf /api/inspect：gotestwaf / blazehttp / waftester / crs-ftw。"""
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Callable

try:
    import yaml
except ImportError:
    yaml = None  # type: ignore

DATASET_ALIASES = {
    "bxxhttp": "blazehttp",
    "blaze": "blazehttp",
    "gtw": "gotestwaf",
    "gotest": "gotestwaf",
    "wt": "waftester",
    "crs": "crs-ftw",
    "ftw": "crs-ftw",
}

DATASET_LABELS = {
    "gotestwaf": "GoTestWAF",
    "blazehttp": "blazehttp",
    "waftester": "WAFtester",
    "crs-ftw": "CRS-FTW",
}

DEFAULT_DATASETS = ("gotestwaf", "waftester", "blazehttp")

FETCH_SCRIPTS = {
    "gotestwaf": "fetch-gotestwaf.sh",
    "blazehttp": "fetch-blazehttp.sh",
    "waftester": "fetch-waftester.sh",
    "crs-ftw": "fetch-crs-ftw.sh",
}

VENDOR_SUBPATH = {
    "gotestwaf": "vendor/gotestwaf",
    "blazehttp": "vendor/blazehttp",
    "waftester": "vendor/waftester",
    "crs-ftw": "vendor/crs-ftw",
}

ENV_ROOT = {
    "gotestwaf": "GOTESTWAF_ROOT",
    "blazehttp": "BLAZEHTTP_ROOT",
    "waftester": "WAFTESTER_ROOT",
    "crs-ftw": "CRS_FTW_ROOT",
}

TMP_ROOT = {
    "gotestwaf": "/tmp/gotestwaf",
    "blazehttp": "/tmp/blazehttp",
    "waftester": "/tmp/waftester",
    "crs-ftw": "/tmp/crs-ftw",
}


@dataclass
class Case:
    id: str
    kind: str  # black | white
    expect: str  # block | pass
    method: str
    host: str
    uri: str
    query: str
    headers: dict[str, str]
    body: str


@dataclass
class DatasetResult:
    dataset: str
    label: str
    total: int = 0
    malicious_total: int = 0
    blocked_ok: int = 0
    missed: int = 0
    normal_total: int = 0
    passed_ok: int = 0
    false_positive: int = 0
    errors: int = 0
    detection_rate: float = 0.0
    false_positive_rate: float = 0.0
    elapsed_s: float = 0.0
    pass_gate: bool = False
    error_message: str = ""
    fn_samples: list[str] = field(default_factory=list)
    fp_samples: list[str] = field(default_factory=list)


# ---------- GoTestWAF YAML 规范化 ----------

def is_yaml_double_quote_escape(c: str) -> bool:
    return c in "0abtnvfre \"/\\N_LPxuU"


def sanitize_yaml_dq_inner(inner: str) -> str:
    out: list[str] = []
    i = 0
    while i < len(inner):
        c = inner[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        if i + 1 >= len(inner):
            out.append("\\")
            break
        nxt = inner[i + 1]
        if is_yaml_double_quote_escape(nxt):
            out.append("\\")
            out.append(nxt)
            i += 2
            hex_len = {"x": 2, "u": 4, "U": 8}.get(nxt, 0)
            for _ in range(hex_len):
                if i < len(inner) and inner[i] in "0123456789abcdefABCDEF":
                    out.append(inner[i])
                    i += 1
                else:
                    break
        else:
            out.append("\\\\")
            out.append(nxt)
            i += 2
    return "".join(out)


def split_double_quoted_list_item(line: str) -> tuple[str, str, str] | None:
    i = 0
    while i < len(line) and line[i] in " \t":
        i += 1
    if i >= len(line) or line[i] != "-":
        return None
    i += 1
    if i >= len(line) or line[i] not in " \t":
        return None
    while i < len(line) and line[i] in " \t":
        i += 1
    if i >= len(line) or line[i] != '"':
        return None
    prefix = line[:i]
    rest = line[i + 1 :]
    j = 0
    while j < len(rest):
        if rest[j] == "\\" and j + 1 < len(rest):
            j += 2
            continue
        if rest[j] == '"':
            return prefix, rest[:j], rest[j + 1 :]
        j += 1
    return None


def sanitize_gotestwaf_yaml(text: str) -> str:
    lines = text.splitlines()
    out: list[str] = []
    for line in lines:
        part = split_double_quoted_list_item(line)
        if part:
            prefix, inner, suffix = part
            out.append(f'{prefix}"{sanitize_yaml_dq_inner(inner)}"{suffix}')
        else:
            out.append(line)
    result = "\n".join(out)
    if text.endswith("\n"):
        result += "\n"
    return result


def encode_payload(payload: str, encoder: str) -> str:
    enc = encoder.lower()
    if enc == "url":
        return urllib.parse.quote(payload, safe="")
    if enc in ("base64", "base64flat"):
        return base64.b64encode(payload.encode()).decode()
    if enc == "base64url":
        return base64.urlsafe_b64encode(payload.encode()).decode().rstrip("=")
    return payload


def placeholder_to_request(ph: Any, encoded: str) -> tuple[str, dict[str, Any]]:
    name = "URLParam"
    spec: Any = None
    if isinstance(ph, str):
        name = ph
    elif isinstance(ph, dict) and ph:
        name = next(iter(ph.keys()))
        spec = ph[name]

    headers = {
        "Host": "gotestwaf.local",
        "User-Agent": "nerve-gotestwaf-runner/1.0",
    }
    if name == "URLPath":
        uri = "/" + encoded.lstrip("/")
        return name, {"method": "GET", "host": "gotestwaf.local", "uri": uri, "query": "", "headers": headers, "body": ""}
    if name == "Header":
        headers["X-Nerve-Test"] = encoded
        return name, {"method": "GET", "host": "gotestwaf.local", "uri": "/", "query": "", "headers": headers, "body": ""}
    if name == "UserAgent":
        headers["User-Agent"] = encoded
        return name, {"method": "GET", "host": "gotestwaf.local", "uri": "/", "query": "", "headers": headers, "body": ""}
    if name in ("JSONRequest", "JSONBody"):
        headers["Content-Type"] = "application/json"
        return name, {
            "method": "POST",
            "host": "gotestwaf.local",
            "uri": "/api",
            "query": "",
            "headers": headers,
            "body": json.dumps({"q": encoded}),
        }
    if name in ("HTMLForm", "RequestBody"):
        headers["Content-Type"] = "application/x-www-form-urlencoded"
        return name, {
            "method": "POST",
            "host": "gotestwaf.local",
            "uri": "/",
            "query": "",
            "headers": headers,
            "body": f"q={encoded}",
        }
    if name == "HTMLMultipartForm":
        boundary = "----NerveBoundary"
        headers["Content-Type"] = f"multipart/form-data; boundary={boundary}"
        body = (
            f"--{boundary}\r\nContent-Disposition: form-data; name=\"q\"\r\n\r\n"
            f"{encoded}\r\n--{boundary}--\r\n"
        )
        return name, {"method": "POST", "host": "gotestwaf.local", "uri": "/", "query": "", "headers": headers, "body": body}
    if name in ("SOAPBody", "XMLBody"):
        headers["Content-Type"] = "text/xml"
        return name, {
            "method": "POST",
            "host": "gotestwaf.local",
            "uri": "/",
            "query": "",
            "headers": headers,
            "body": f"<root><q>{encoded}</q></root>",
        }
    if name == "RawRequest" and isinstance(spec, dict):
        method = str(spec.get("method") or spec.get("Method") or "POST")
        path = str(spec.get("path") or spec.get("Path") or "/")
        body_tpl = str(spec.get("body") or spec.get("Body") or "{{payload}}")
        body = body_tpl.replace("{{payload}}", encoded)
        hdrs = spec.get("headers") or spec.get("Headers")
        if isinstance(hdrs, dict):
            for k, v in hdrs.items():
                headers[str(k)] = str(v)
        return name, {
            "method": method,
            "host": "gotestwaf.local",
            "uri": path,
            "query": "",
            "headers": headers,
            "body": body,
        }
    return name, {
        "method": "GET",
        "host": "gotestwaf.local",
        "uri": "/",
        "query": f"q={encoded}",
        "headers": headers,
        "body": "",
    }


def collect_yaml_files(root: Path) -> list[Path]:
    out: list[Path] = []
    for path in sorted(root.rglob("*.yml")):
        out.append(path)
    for path in sorted(root.rglob("*.yaml")):
        if path not in out:
            out.append(path)
    return out


def collect_json_files(root: Path) -> list[Path]:
    return sorted(p for p in root.rglob("*.json") if p.is_file())


# ---------- raw HTTP（blazehttp / waftester） ----------

def split_request_target(target: str) -> tuple[str, str]:
    if "?" in target:
        uri, query = target.split("?", 1)
        return uri or "/", query
    return target or "/", ""


def parse_raw_http(raw: str) -> dict[str, Any]:
    normalized = raw.replace("\r\n", "\n").replace("\r", "\n")
    head, body = (normalized.split("\n\n", 1) + [""])[:2]
    lines = head.split("\n")
    if not lines or not lines[0].strip():
        raise ValueError("empty request")
    parts = lines[0].split()
    method = parts[0] if parts else "GET"
    target = parts[1] if len(parts) > 1 else "/"
    uri, query = split_request_target(target)
    headers: dict[str, str] = {}
    host = "blazehttp.local"
    for line in lines[1:]:
        if ":" not in line:
            continue
        k, v = line.split(":", 1)
        key = k.strip()
        val = v.strip()
        if not key:
            key = "_empty"
        if key.lower() == "host":
            host = val
        headers[key] = val
    if "Host" not in headers and "host" not in {k.lower() for k in headers}:
        headers["Host"] = host
    return {
        "method": method,
        "host": host,
        "uri": uri,
        "query": query,
        "headers": headers,
        "body": body,
    }


def case_from_req(cid: str, kind: str, req: dict[str, Any]) -> Case:
    expect = "block" if kind == "black" else "pass"
    return Case(
        id=cid,
        kind=kind,
        expect=expect,
        method=str(req.get("method") or "GET"),
        host=str(req.get("host") or "test.local"),
        uri=str(req.get("uri") or "/"),
        query=str(req.get("query") or ""),
        headers={str(k): str(v) for k, v in (req.get("headers") or {}).items()},
        body=str(req.get("body") or ""),
    )


# ---------- waftester payload 路由（对齐 dashboard 主路径） ----------

HEADER_INJECT_NAMES = frozenset(
    n.lower()
    for n in (
        "user-agent",
        "referer",
        "cookie",
        "x-forwarded-for",
        "x-real-ip",
        "host",
        "x-original-url",
        "x-rewrite-url",
    )
)


def split_single_header_line(p: str) -> tuple[str, str] | None:
    if "\n" in p or "\r" in p:
        return None
    if ":" not in p:
        return None
    name, val = p.split(":", 1)
    name = name.strip()
    val = val.strip()
    if not name or not val or " " in name or len(name) > 64:
        return None
    if name.lower() not in HEADER_INJECT_NAMES:
        return None
    return name, val


def looks_like_jwt_payload(p: str) -> bool:
    t = p.strip()
    return t.count(".") >= 2 and len(t) > 20 and " " not in t


def looks_like_path_payload(p: str) -> bool:
    t = p.strip()
    if len(t) < 4 or len(t) > 512:
        return False
    if " " in t and "../" not in t and "..\\" not in t:
        return False
    lower = t.lower()
    markers = (
        "../",
        "..\\",
        "..../",
        "/etc/",
        "/var/",
        "/tmp/",
        "/proc/",
        "etc/passwd",
        "web.config",
        "%2e%2e",
        "%c0%af",
    )
    return any(m in lower for m in markers) or lower.startswith(("..", "/etc", "/var", "/tmp", "c:\\", "c:/"))


def looks_like_uri_path_payload(p: str) -> bool:
    t = p.strip()
    if not t.startswith("/") or t.startswith("//") or len(t) < 2 or len(t) > 2048:
        return False
    if "\n" in t or "\r" in t:
        return False
    path = t.split("?", 1)[0]
    return all(
        c.isalnum() or c in "/-_.~%:@+=<>,"
        for c in path
    )


def looks_like_json_payload(p: str) -> bool:
    t = p.lstrip()
    if len(t) < 2 or len(t) > 65536:
        return False
    return (t.startswith("{") or t.startswith("[")) and (":" in t or "query" in t or "$" in t)


def looks_like_http2_pseudo_block(p: str) -> bool:
    t = p.strip()
    return t.startswith(":method ") or (
        t.startswith(":") and (":path " in t or ":authority " in t)
    )


def waftester_http2_pseudo_to_request(payload: str) -> dict[str, Any]:
    headers: dict[str, str] = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
    for line in payload.replace("\r", "\n").splitlines():
        line = line.strip()
        if not line:
            continue
        if line.startswith(":"):
            rest = line[1:]
            if " " in rest:
                key, val = rest.split(" ", 1)
                headers[f":{key.strip()}"] = val.strip()
            continue
        if ":" in line:
            k, v = line.split(":", 1)
            k, v = k.strip(), v.strip()
            if k and v:
                headers[k] = v
    method = headers.get(":method", "GET")
    uri = headers.get(":path", "/")
    host = headers.get(":authority", "waftester.local")
    return {"method": method, "host": host, "uri": uri, "query": "", "headers": headers, "body": ""}


def waftester_payload_to_request(payload: str) -> dict[str, Any]:
    p = payload.strip()
    if re.match(r"^(GET|POST|PUT|DELETE|HEAD|PATCH)\s", p, re.I):
        raw = payload if ("\r\n\r\n" in payload or "\n\n" in payload) else f"{payload}\r\n\r\n"
        try:
            return parse_raw_http(raw)
        except ValueError:
            pass
    if looks_like_http2_pseudo_block(p):
        return waftester_http2_pseudo_to_request(p)
    hdr = split_single_header_line(p)
    if hdr:
        name, val = hdr
        headers = {"Host": "waftester.local", name: val}
        if name.lower() != "user-agent":
            headers["User-Agent"] = "nerve-waftester-runner/1.0"
        return {"method": "GET", "host": "waftester.local", "uri": "/", "query": "", "headers": headers, "body": ""}
    if "/???/" in p or "/????/" in p or (p.startswith(";/") and "???" in p):
        headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
        if p.startswith(";"):
            return {"method": "GET", "host": "waftester.local", "uri": "/", "query": f"q={p}", "headers": headers, "body": ""}
        uri = p if p.startswith("/") else f"/{p}"
        return {"method": "GET", "host": "waftester.local", "uri": uri, "query": "", "headers": headers, "body": ""}
    if looks_like_jwt_payload(p):
        headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
        return {
            "method": "GET",
            "host": "waftester.local",
            "uri": "/api",
            "query": f"access_token={urllib.parse.quote(p, safe='')}",
            "headers": headers,
            "body": "",
        }
    if looks_like_path_payload(p):
        headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
        uri = p if p.startswith("/") else f"/{p}"
        return {"method": "GET", "host": "waftester.local", "uri": uri, "query": "", "headers": headers, "body": ""}
    if looks_like_uri_path_payload(p):
        headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
        uri, query = split_request_target(p)
        return {"method": "GET", "host": "waftester.local", "uri": uri, "query": query, "headers": headers, "body": ""}
    if looks_like_json_payload(p):
        headers = {
            "Host": "waftester.local",
            "User-Agent": "nerve-waftester-runner/1.0",
            "Content-Type": "application/json",
        }
        low = p.lower()
        uri = "/graphql" if "__schema" in low or "__type" in low or '"query"' in p else "/api"
        return {"method": "POST", "host": "waftester.local", "uri": uri, "query": "", "headers": headers, "body": p}
    headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
    return {
        "method": "GET",
        "host": "waftester.local",
        "uri": "/",
        "query": f"q={urllib.parse.quote(payload, safe='')}",
        "headers": headers,
        "body": "",
    }


def waftester_item_to_request(item: dict[str, Any]) -> dict[str, Any]:
    body = str(item.get("body") or "").strip()
    if body:
        path = str(item.get("endpoint") or item.get("payload") or "").strip() or "/"
        uri, query = split_request_target(path if path.startswith("/") else f"/{path}")
        headers = {"Host": "waftester.local", "User-Agent": "nerve-waftester-runner/1.0"}
        method = str(item.get("method") or "POST").upper()
        if method == "POST":
            headers.setdefault("Content-Type", "application/json")
        for k, v in (item.get("headers") or {}).items():
            headers[str(k)] = str(v)
        return {"method": method, "host": "waftester.local", "uri": uri, "query": query, "headers": headers, "body": body}
    req = waftester_payload_to_request(str(item.get("payload") or ""))
    if str(item.get("method") or "").strip():
        req["method"] = str(item["method"]).strip().upper()
    for k, v in (item.get("headers") or {}).items():
        req.setdefault("headers", {})[str(k)] = str(v)
    return req


# ---------- 数据集加载 ----------

def load_gotestwaf_cases(root: Path, limit: int = 0) -> list[Case]:
    require_yaml()
    tc = root / "testcases"
    if not tc.is_dir():
        raise RuntimeError(f"缺少 GoTestWAF testcases: {tc}")

    cases: list[Case] = []
    skipped = 0
    for path in collect_yaml_files(tc):
        rel = path.relative_to(root).as_posix()
        is_fp = "false-pos" in rel or "false_pos" in rel
        text = sanitize_gotestwaf_yaml(path.read_text(encoding="utf-8", errors="replace"))
        try:
            doc = yaml.safe_load(text) or {}
        except yaml.YAMLError as e:
            skipped += 1
            print(f"[gotestwaf] 跳过无法解析: {rel} ({e})", flush=True)
            continue
        if not isinstance(doc, dict):
            skipped += 1
            continue
        payloads = doc.get("payload") or []
        encoders = doc.get("encoder") or ["Plain"]
        placeholders = doc.get("placeholder") or ["URLParam"]
        idx = 0
        for payload in payloads:
            for enc in encoders:
                encoded = encode_payload(str(payload), str(enc))
                for ph in placeholders:
                    ph_name, req = placeholder_to_request(ph, encoded)
                    idx += 1
                    cid = f"{rel}#{idx}/{enc}/{ph_name}"
                    kind = "white" if is_fp else "black"
                    cases.append(case_from_req(cid, kind, req))
                    if limit > 0 and len(cases) >= limit:
                        return cases
    if skipped:
        print(f"[gotestwaf] 已跳过 {skipped} 个无法解析的文件", flush=True)
    if not cases:
        raise RuntimeError("GoTestWAF 用例为空")
    return cases


def load_blazehttp_cases(root: Path, limit: int = 0) -> list[Case]:
    tc = root / "testcases"
    if not tc.is_dir():
        raise RuntimeError(f"缺少 blazehttp testcases: {tc}")
    files = sorted(
        p
        for p in tc.rglob("*")
        if p.is_file() and (p.name.endswith(".black") or p.name.endswith(".white"))
    )
    cases: list[Case] = []
    skipped = 0
    for path in files:
        kind = "black" if path.name.endswith(".black") else "white"
        raw = path.read_bytes().decode("utf-8", errors="replace")
        try:
            req = parse_raw_http(raw)
        except ValueError as e:
            skipped += 1
            if skipped <= 5:
                print(f"[blazehttp] 跳过 {path.relative_to(root)}: {e}", flush=True)
            continue
        rel = path.relative_to(root).as_posix()
        cases.append(case_from_req(rel, kind, req))
        if limit > 0 and len(cases) >= limit:
            break
    if skipped:
        print(f"[blazehttp] 已跳过 {skipped} 个无法解析的样本", flush=True)
    if not cases:
        raise RuntimeError("blazehttp 用例为空")
    return cases


def load_waftester_cases(root: Path, limit: int = 0) -> list[Case]:
    payloads = root / "payloads"
    if not payloads.is_dir():
        raise RuntimeError(f"缺少 waftester payloads: {payloads}")
    cases: list[Case] = []
    for path in collect_json_files(payloads):
        if path.name == "ids-map.json":
            continue
        try:
            items = json.loads(path.read_text(encoding="utf-8", errors="replace"))
        except json.JSONDecodeError as e:
            print(f"[waftester] 跳过 {path.name}: {e}", flush=True)
            continue
        if not isinstance(items, list):
            continue
        rel = path.relative_to(root).as_posix()
        for i, it in enumerate(items):
            if not isinstance(it, dict):
                continue
            payload = str(it.get("payload") or "").strip()
            if not payload and not str(it.get("body") or "").strip():
                continue
            if int(it.get("repeat") or 1) > 1:
                continue
            expect_block = it.get("expected_block", True)
            if expect_block is None:
                expect_block = True
            kind = "black" if expect_block else "white"
            cid = str(it.get("id") or f"{rel}#{i}")
            req = waftester_item_to_request(it)
            cases.append(case_from_req(f"{rel}/{cid}", kind, req))
            if limit > 0 and len(cases) >= limit:
                return cases
    if not cases:
        raise RuntimeError("WAFtester 用例为空")
    return cases


def yaml_scalar(v: Any) -> str:
    if v is None:
        return ""
    return str(v)


def crs_tests_dir(root: Path) -> Path:
    for sub in (
        "tests/regression/tests",
        "coreruleset/tests/regression/tests",
    ):
        p = root / sub
        if p.is_dir():
            return p
    return root / "tests/regression/tests"


def load_crs_ftw_cases(root: Path, limit: int = 0) -> list[Case]:
    require_yaml()
    tests_dir = crs_tests_dir(root)
    if not tests_dir.is_dir():
        raise RuntimeError(f"缺少 CRS FTW tests: {tests_dir}")
    cases: list[Case] = []
    for path in collect_yaml_files(tests_dir):
        rel = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8", errors="replace").lstrip("\ufeff").lstrip("-").lstrip()
        try:
            doc = yaml.safe_load(text) or {}
        except yaml.YAMLError:
            continue
        if not isinstance(doc, dict):
            continue
        rule = yaml_scalar(doc.get("rule_id"))
        for t in doc.get("tests") or []:
            if not isinstance(t, dict):
                continue
            stages = t.get("stages") or []
            if not stages or not isinstance(stages[0], dict):
                continue
            stage = stages[0]
            inp = stage.get("input") or {}
            out = stage.get("output") or {}
            if not isinstance(inp, dict):
                inp = {}
            if not isinstance(out, dict):
                out = {}
            log = out.get("log") or {}
            expect_ids = log.get("expect_ids") if isinstance(log, dict) else []
            status = out.get("status")
            expect_block = bool(expect_ids) or status in (403, 406)
            is_negative = out.get("no_log") is not None and not expect_block
            method = str(inp.get("method") or "GET").upper()
            uri_raw = str(inp.get("uri") or "/")
            uri, query = split_request_target(uri_raw)
            headers = {str(k): str(v) for k, v in (inp.get("headers") or {}).items()}
            host = str(inp.get("dest_addr") or headers.get("Host") or "crs-ftw.local")
            headers.setdefault("Host", host)
            tid = yaml_scalar(t.get("test_id"))
            cid = f"{rel}#r{rule}-t{tid}"
            kind = "black" if expect_block and not is_negative else "white"
            req = {
                "method": method,
                "host": host,
                "uri": uri,
                "query": query,
                "headers": headers,
                "body": str(inp.get("data") or ""),
            }
            cases.append(case_from_req(cid, kind, req))
            if limit > 0 and len(cases) >= limit:
                return cases
    if not cases:
        raise RuntimeError("CRS-FTW 用例为空")
    return cases


LOADERS: dict[str, Callable[[Path, int], list[Case]]] = {
    "gotestwaf": load_gotestwaf_cases,
    "blazehttp": load_blazehttp_cases,
    "waftester": load_waftester_cases,
    "crs-ftw": load_crs_ftw_cases,
}


def require_yaml() -> None:
    if yaml is None:
        raise RuntimeError("缺少 PyYAML: pip install pyyaml 或 apt install python3-yaml")


def normalize_dataset(name: str) -> str:
    key = name.strip().lower()
    return DATASET_ALIASES.get(key, key)


def parse_datasets(raw: str) -> list[str]:
    out: list[str] = []
    for part in raw.split(","):
        ds = normalize_dataset(part)
        if ds and ds not in out:
            out.append(ds)
    return out


def resolve_dataset_root(dataset: str, native_root: Path) -> Path:
    env_key = ENV_ROOT.get(dataset, "")
    if env_key and os.environ.get(env_key):
        p = Path(os.environ[env_key])
        if p.is_dir():
            return p.resolve()
    sub = VENDOR_SUBPATH.get(dataset, "")
    if sub:
        p = native_root / sub
        try:
            if p.is_dir():
                return p.resolve()
        except OSError:
            pass
    tmp = TMP_ROOT.get(dataset)
    if tmp and Path(tmp).is_dir():
        return Path(tmp).resolve()
    return Path()


def ensure_dataset(dataset: str, native_root: Path) -> Path:
    root = resolve_dataset_root(dataset, native_root)
    # Path() == Path('.')，不能当「已找到」——否则会落到 cwd 下假路径
    if root != Path() and root.is_dir():
        return root
    script = FETCH_SCRIPTS.get(dataset)
    if not script:
        raise RuntimeError(f"未知数据集: {dataset}")
    fetch = native_root / "scripts" / script
    if not fetch.is_file():
        waf_fetch = native_root.parent / "Nerve-waf" / "scripts" / script
        if waf_fetch.is_file():
            fetch = waf_fetch
        else:
            raise RuntimeError(f"未找到 {dataset} 数据集，且缺少 {script}")
    print(f"[{dataset}] 拉取数据集: {fetch}", flush=True)
    subprocess.run(["bash", str(fetch)], check=True)
    root = resolve_dataset_root(dataset, native_root)
    if root == Path() or not root.is_dir():
        raise RuntimeError(f"fetch 后仍未找到 {dataset} 数据集")
    return root


def load_cases(dataset: str, native_root: Path, limit: int) -> list[Case]:
    loader = LOADERS.get(dataset)
    if not loader:
        raise RuntimeError(f"不支持的数据集: {dataset}")
    root = ensure_dataset(dataset, native_root)
    print(f"[{dataset}] root={root}", flush=True)
    return loader(root, limit)


# ---------- inspect 跑表 ----------

def to_inspect_payload(case: Case) -> dict[str, Any]:
    uri = case.uri
    if case.query:
        q = case.query.lstrip("?")
        uri = f"{uri}&{q}" if "?" in uri else f"{uri}?{q}"
    return {
        "method": case.method,
        "host": case.host,
        "uri": uri,
        "headers": case.headers,
        "body": case.body,
        "src_ip": "10.0.0.99",
        "persist": False,
    }


def case_uri(case: Case) -> str:
    uri = case.uri or "/"
    if case.query:
        q = case.query.lstrip("?")
        uri = f"{uri}&{q}" if "?" in uri else f"{uri}?{q}"
    return uri


def inspect_one(api: str, case: Case, timeout: float) -> tuple[Case, str, bool, str]:
    payload = json.dumps(to_inspect_payload(case)).encode()
    req = urllib.request.Request(
        f"{api.rstrip('/')}/api/inspect",
        data=payload,
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            out = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")
        return case, "error", False, f"HTTP {e.code}: {body[:120]}"
    except Exception as e:
        return case, "error", False, str(e)

    blocked = bool(out.get("blocked"))
    actual = "block" if blocked else "pass"
    ok = actual == case.expect
    hit = out.get("hit") or {}
    detail = str(hit.get("rule_id") or hit.get("category") or "") if hit else ""
    return case, actual, ok, detail


def edge_one(edge: str, edge_host: str, case: Case, timeout: float) -> tuple[Case, str, bool, str]:
    """形态 B：HTTP 打 nerve-waf-edge；403 / blocked JSON 视为拦截。"""
    url = f"{edge.rstrip('/')}{case_uri(case)}"
    host = (edge_host or case.host or "test.local").strip()
    headers = {k: str(v) for k, v in (case.headers or {}).items()}
    headers["Host"] = host
    data = case.body.encode() if case.body and case.method.upper() not in ("GET", "HEAD") else None
    req = urllib.request.Request(url, data=data, method=case.method.upper(), headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            code = resp.getcode()
            body = resp.read(4096).decode(errors="replace")
    except urllib.error.HTTPError as e:
        code = e.code
        body = e.read(4096).decode(errors="replace")
    except Exception as e:
        return case, "error", False, str(e)

    blocked = code == 403
    if not blocked and '"blocked"' in body and "true" in body.lower():
        blocked = True
    actual = "block" if blocked else "pass"
    ok = actual == case.expect
    detail = f"HTTP {code}"
    return case, actual, ok, detail


def run_cases(
    target: str,
    dataset: str,
    cases: list[Case],
    concurrency: int,
    timeout: float,
    min_detection: float,
    max_fp: float,
    *,
    mode: str = "inspect",
    edge_host: str = "",
) -> DatasetResult:
    label = DATASET_LABELS.get(dataset, dataset)
    res = DatasetResult(dataset=dataset, label=label, total=len(cases))
    if not cases:
        res.error_message = "用例为空"
        return res

    t0 = time.time()
    results: list[tuple[Case, str, bool, str]] = []

    def probe(c: Case) -> tuple[Case, str, bool, str]:
        if mode == "edge":
            return edge_one(target, edge_host, c, timeout)
        return inspect_one(target, c, timeout)

    with ThreadPoolExecutor(max_workers=max(1, concurrency)) as pool:
        futs = [pool.submit(probe, c) for c in cases]
        done = 0
        for fut in as_completed(futs):
            results.append(fut.result())
            done += 1
            if done % 500 == 0 or done == len(cases):
                print(f"  [{dataset}] progress {done}/{len(cases)}", flush=True)

    res.elapsed_s = time.time() - t0
    mal = [r for r in results if r[0].kind == "black"]
    ben = [r for r in results if r[0].kind == "white"]
    res.malicious_total = len(mal)
    res.normal_total = len(ben)
    res.blocked_ok = sum(1 for r in mal if r[1] == "block")
    res.passed_ok = sum(1 for r in ben if r[1] == "pass")
    res.missed = sum(1 for r in mal if r[1] != "block")
    res.false_positive = sum(1 for r in ben if r[1] == "block")
    res.errors = sum(1 for r in results if r[1] == "error")
    res.detection_rate = res.blocked_ok / res.malicious_total if res.malicious_total else 1.0
    res.false_positive_rate = res.false_positive / res.normal_total if res.normal_total else 0.0
    res.fn_samples = [r[0].id[:100] for r in results if r[0].kind == "black" and r[1] != "block"][:5]
    res.fp_samples = [r[0].id[:100] for r in results if r[0].kind == "white" and r[1] == "block"][:5]
    res.pass_gate = (
        res.detection_rate >= min_detection
        and res.false_positive_rate <= max_fp
        and res.errors == 0
    )
    return res


def print_summary_table(results: list[DatasetResult], api: str) -> None:
    print("", flush=True)
    print(f"WAF 评测汇总  api={api}", flush=True)
    header = (
        f"{'数据集':14} {'用例':>7} {'检出%':>8} {'误报%':>8} "
        f"{'FN':>6} {'FP':>5} {'err':>4} {'耗时s':>8} {'结果':>6}"
    )
    print(header, flush=True)
    print("-" * len(header.encode("utf-8")), flush=True)
    for r in results:
        if r.error_message:
            print(f"{r.label:14} {'—':>7} {'—':>8} {'—':>8} {'—':>6} {'—':>5} {'—':>4} {'—':>8} ERR", flush=True)
            print(f"  ! {r.error_message}", flush=True)
            continue
        flag = "PASS" if r.pass_gate else "FAIL"
        print(
            f"{r.label:14} {r.total:7d} {r.detection_rate * 100:8.2f} {r.false_positive_rate * 100:8.2f} "
            f"{r.missed:6d} {r.false_positive:5d} {r.errors:4d} {r.elapsed_s:8.1f} {flag:>6}",
            flush=True,
        )
    print("-" * len(header.encode("utf-8")), flush=True)
    passed = sum(1 for r in results if r.pass_gate and not r.error_message)
    failed = sum(1 for r in results if not r.pass_gate and not r.error_message)
    errored = sum(1 for r in results if r.error_message)
    print(f"通过 {passed} / 失败 {failed} / 错误 {errored} / 合计 {len(results)}", flush=True)


def write_report(path: Path, api: str, results: list[DatasetResult], args: argparse.Namespace) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "api": api,
        "datasets": [r.dataset for r in results],
        "min_detection": args.min_detection,
        "max_fp": args.max_fp,
        "concurrency": args.concurrency,
        "limit": args.limit,
        "rows": [asdict(r) for r in results],
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"报告: {path}", flush=True)


def build_parser(single_dataset: str | None = None) -> argparse.ArgumentParser:
    default_ds = single_dataset or os.environ.get(
        "NERVE_WAF_BENCH_DATASETS",
        os.environ.get("NERVE_GOTESTWAF_DATASETS", ",".join(DEFAULT_DATASETS)),
    )
    parser = argparse.ArgumentParser(
        description="WAF 评测集 → Nerve-waf /api/inspect（gotestwaf / blazehttp / waftester / crs-ftw）",
    )
    parser.add_argument("--api", default=os.environ.get("NERVE_WAF_EXPERT_API", "http://127.0.0.1:8782"))
    parser.add_argument(
        "--edge",
        default=os.environ.get("WAF_EDGE_STACK_EDGE", os.environ.get("NERVE_WAF_EDGE_URL", "")),
        help="形态 B：打 nerve-waf-edge 基址（设置后走真实流量，不用 /api/inspect）",
    )
    parser.add_argument(
        "--edge-host",
        default=os.environ.get("WAF_EDGE_BENCH_HOST", os.environ.get("NERVE_WAF_EDGE_BENCH_HOST", "test.local")),
        help="Edge 请求 Host 头（需与 dataplane 路由匹配；* 兜底时可任意）",
    )
    parser.add_argument(
        "--datasets",
        default=default_ds,
        help="逗号分隔：gotestwaf,waftester,blazehttp,crs-ftw（别名 bxxhttp→blazehttp）",
    )
    parser.add_argument("--limit", type=int, default=int(os.environ.get("NERVE_WAF_BENCH_LIMIT", os.environ.get("NERVE_GOTESTWAF_LIMIT", "0"))))
    parser.add_argument(
        "--concurrency",
        type=int,
        default=int(os.environ.get("NERVE_WAF_BENCH_CONCURRENCY", os.environ.get("NERVE_GOTESTWAF_CONCURRENCY", "40"))),
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=float(os.environ.get("NERVE_WAF_BENCH_TIMEOUT", os.environ.get("NERVE_GOTESTWAF_TIMEOUT", "30"))),
    )
    parser.add_argument(
        "--min-detection",
        type=float,
        default=float(os.environ.get("NERVE_WAF_BENCH_MIN_DETECTION", os.environ.get("NERVE_GOTESTWAF_MIN_DETECTION", "0.80"))),
    )
    parser.add_argument(
        "--max-fp",
        type=float,
        default=float(os.environ.get("NERVE_WAF_BENCH_MAX_FP", os.environ.get("NERVE_GOTESTWAF_MAX_FP", "0.10"))),
    )
    parser.add_argument("--report", default=os.environ.get("NERVE_WAF_BENCH_REPORT", ""))
    return parser


def main(argv: list[str] | None = None, *, single_dataset: str | None = None) -> int:
    if yaml is None and single_dataset != "blazehttp":
        print("缺少 PyYAML: pip install pyyaml 或 apt install python3-yaml", file=sys.stderr)
        return 2

    script_dir = Path(__file__).resolve().parent.parent
    waf_root = script_dir.parent
    native_root = Path(os.environ.get("NATIVE_ROOT", waf_root.parent / "Nerve-waf-native"))

    parser = build_parser(single_dataset)
    args = parser.parse_args(argv)

    datasets = [single_dataset] if single_dataset else parse_datasets(args.datasets)
    for ds in datasets:
        if ds not in LOADERS:
            print(f"未知数据集: {ds}（支持: {', '.join(LOADERS)}）", file=sys.stderr)
            return 2

    edge_url = (args.edge or "").strip()
    mode = "edge" if edge_url else "inspect"
    bench_target = edge_url if mode == "edge" else args.api.rstrip("/")

    if mode == "edge":
        health = f"{bench_target.rstrip('/')}/health"
    else:
        health = f"{bench_target.rstrip('/')}/api/health"
    try:
        urllib.request.urlopen(health, timeout=5)
    except Exception as e:
        print(f"目标未就绪: {health} ({e})", file=sys.stderr)
        return 1

    results: list[DatasetResult] = []
    for ds in datasets:
        label = DATASET_LABELS.get(ds, ds)
        print(f"\n==> {label} ({ds})", flush=True)
        try:
            cases = load_cases(ds, native_root, args.limit)
            tag = f"edge={bench_target} host={args.edge_host}" if mode == "edge" else f"api={bench_target}"
            print(f"[{ds}] cases={len(cases)} {tag} conc={args.concurrency}", flush=True)
            results.append(
                run_cases(
                    bench_target,
                    ds,
                    cases,
                    args.concurrency,
                    args.timeout,
                    args.min_detection,
                    args.max_fp,
                    mode=mode,
                    edge_host=args.edge_host,
                )
            )
        except Exception as e:
            print(f"[{ds}] 错误: {e}", file=sys.stderr)
            results.append(DatasetResult(dataset=ds, label=label, error_message=str(e)))

    print_summary_table(results, bench_target)

    for r in results:
        if r.fn_samples:
            print(f"\n[{r.label}] FN 样例:", flush=True)
            for s in r.fn_samples:
                print(f"  {s}", flush=True)
        if r.fp_samples:
            print(f"\n[{r.label}] FP 样例:", flush=True)
            for s in r.fp_samples:
                print(f"  {s}", flush=True)

    default_report = (
        str(waf_root / "logs/waf-edge-stack-bench.json")
        if mode == "edge"
        else str(waf_root / "logs/nerve-waf-expert-bench.json")
    )
    report_path = Path(args.report or default_report)
    write_report(report_path, bench_target, results, args)

    ok = all(r.pass_gate and not r.error_message for r in results)
    if not ok:
        print(
            f"\n未全部达标: 检出率>={args.min_detection:.0%} 误报率<={args.max_fp:.0%} 无错误",
            file=sys.stderr,
        )
        return 1
    print("\n全部评测通过", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
