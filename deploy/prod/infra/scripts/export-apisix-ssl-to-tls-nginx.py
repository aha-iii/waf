#!/usr/bin/env python3
"""从 APISIX Admin 导出 SSL，生成 tls/nginx.conf（在 infra 目录执行）。"""
import json
import os
import re
import urllib.request

admin = os.environ.get("APISIX_ADMIN", "http://127.0.0.1:9180")
key = os.environ.get("APISIX_ADMIN_KEY", "edd1c9f034335f136f87ad84b625c8f1")
if os.path.isfile(".env"):
    for line in open(".env", encoding="utf-8"):
        if line.startswith("APISIX_ADMIN_KEY="):
            key = line.split("=", 1)[1].strip()

req = urllib.request.Request(admin + "/apisix/admin/ssls", headers={"X-API-KEY": key})
try:
    with urllib.request.urlopen(req, timeout=30) as r:
        raw = r.read().decode("utf-8", errors="replace")
        data = json.loads(raw)
except Exception as e:
    raise SystemExit(f"GET {admin}/apisix/admin/ssls failed: {e}") from e

items = data.get("list")
if items is None and isinstance(data.get("node"), dict):
    node = data["node"]
    items = node.get("nodes") or ([node] if node.get("value") else [])
if not isinstance(items, list):
    raise SystemExit(f"unexpected ssls payload keys={list(data.keys())} body={raw[:500]}")

os.makedirs("certs", exist_ok=True)
blocks = []
for item in items:
    v = item.get("value") or item
    if isinstance(v, str):
        try:
            v = json.loads(v)
        except json.JSONDecodeError:
            continue
    sid = v.get("id") or item.get("key") or ""
    # list 接口常省略 private key，按 id 再拉详情
    if sid and (not v.get("key") or not v.get("cert")):
        detail_req = urllib.request.Request(
            f"{admin}/apisix/admin/ssls/{sid}", headers={"X-API-KEY": key}
        )
        with urllib.request.urlopen(detail_req, timeout=30) as dr:
            detail = json.loads(dr.read().decode("utf-8", errors="replace"))
        v = detail.get("value") or detail.get("node", {}).get("value") or detail
    snis = v.get("snis") or []
    cert = v.get("cert") or ""
    pkey = v.get("key") or ""
    if not snis or not cert or not pkey:
        print("skip incomplete", sid, "snis=", snis, "cert=", bool(cert), "key=", bool(pkey))
        continue
    primary = snis[0]
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", primary)
    pem = f"certs/{safe}.pem"
    keyf = f"certs/{safe}.key"
    open(pem, "w", encoding="utf-8").write(cert if cert.endswith("\n") else cert + "\n")
    open(keyf, "w", encoding="utf-8").write(pkey if pkey.endswith("\n") else pkey + "\n")
    os.chmod(keyf, 0o600)
    blocks.append((" ".join(snis), safe))
    print("exported", safe, " ".join(snis))

if not blocks:
    raise SystemExit(f"no ssl in apisix (items={len(items)})")

servers = []
for names, safe in blocks:
    servers.append(
        f"""
    server {{
        listen 443 ssl;
        listen [::]:443 ssl;
        http2 on;
        server_name {names};
        ssl_certificate     /etc/nginx/certs/{safe}.pem;
        ssl_certificate_key /etc/nginx/certs/{safe}.key;
        ssl_protocols       TLSv1.2 TLSv1.3;
        client_max_body_size 256m;
        location / {{
            proxy_http_version 1.1;
            proxy_set_header Host              $host;
            proxy_set_header X-Real-IP         $remote_addr;
            proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto https;
            proxy_set_header Upgrade           $http_upgrade;
            proxy_set_header Connection        $connection_upgrade;
            proxy_connect_timeout 5s;
            proxy_send_timeout    300s;
            proxy_read_timeout    300s;
            proxy_pass http://nerve_ddos;
        }}
    }}"""
    )

conf = f"""worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;
events {{ worker_connections 4096; }}
http {{
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    sendfile on;
    keepalive_timeout 65;
    client_max_body_size 256m;
    map $http_upgrade $connection_upgrade {{
        default upgrade;
        "" close;
    }}
    upstream nerve_ddos {{
        server nerve-ddos-edge:8780;
        keepalive 64;
    }}
    server {{
        listen 80 default_server;
        listen [::]:80 default_server;
        server_name _;
        return 301 https://$host$request_uri;
    }}
{"".join(servers)}
}}
"""
os.makedirs("tls", exist_ok=True)
open("tls/nginx.conf", "w", encoding="utf-8").write(conf)
print("nginx servers", len(blocks))
