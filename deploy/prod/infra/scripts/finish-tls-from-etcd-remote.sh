#!/usr/bin/env bash
set -euo pipefail
cd "$1"
docker exec -e ETCDCTL_API=3 nerve-prod-etcd etcdctl --endpoints=http://127.0.0.1:2379 get /apisix/ssls --prefix -w json > /tmp/apisix-ssls.json
python3 <<'PY'
import json, os, re, base64, textwrap, subprocess

def wrap_pem(body: str, kind: str) -> str:
    body = body.strip().replace("\n", "").replace("\r", "")
    if "BEGIN" in body:
        return body if body.endswith("\n") else body + "\n"
    lines = textwrap.wrap(body, 64)
    return "-----BEGIN %s-----\n%s\n-----END %s-----\n" % (kind, "\n".join(lines), kind)

d = json.load(open("/tmp/apisix-ssls.json"))
os.makedirs("certs", exist_ok=True)
blocks = []
for kv in d.get("kvs") or []:
    val = kv.get("value")
    if not val:
        continue
    try:
        v = json.loads(base64.b64decode(val))
    except Exception:
        continue
    if not isinstance(v, dict):
        continue
    snis = v.get("snis") or []
    cert = v.get("cert") or ""
    pkey = v.get("key") or ""
    if not snis or not cert or not pkey:
        continue
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", snis[0])
    # cert 已是 PEM；key 可能是裸 base64
    cert_pem = cert if "BEGIN CERTIFICATE" in cert else wrap_pem(cert, "CERTIFICATE")
    key_pem = None
    for kind in ("PRIVATE KEY", "RSA PRIVATE KEY", "EC PRIVATE KEY"):
        candidate = pkey if "BEGIN" in pkey else wrap_pem(pkey, kind)
        open(f"certs/{safe}.key", "w", encoding="utf-8").write(candidate)
        os.chmod(f"certs/{safe}.key", 0o600)
        chk = subprocess.run(["openssl", "pkey", "-in", f"certs/{safe}.key", "-noout"], capture_output=True)
        if chk.returncode == 0:
            key_pem = candidate
            break
    if not key_pem:
        print("skip undecodable key", safe)
        continue
    open(f"certs/{safe}.pem", "w", encoding="utf-8").write(cert_pem if cert_pem.endswith("\n") else cert_pem + "\n")
    open(f"certs/{safe}.key", "w", encoding="utf-8").write(key_pem if key_pem.endswith("\n") else key_pem + "\n")
    blocks.append((" ".join(snis), safe))
    print("exported", safe)

if not blocks:
    raise SystemExit("no usable ssl")

tpl_head = r"""worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;
events { worker_connections 4096; }
http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    sendfile on;
    keepalive_timeout 65;
    client_max_body_size 256m;
    map $http_upgrade $connection_upgrade {
        default upgrade;
        "" close;
    }
    upstream nerve_ddos {
        server nerve-ddos-edge:8780;
        keepalive 64;
    }
    server {
        listen 80 default_server;
        listen [::]:80 default_server;
        server_name _;
        return 301 https://$host$request_uri;
    }
"""
servers = []
for names, safe in blocks:
    servers.append("""
    server {
        listen 443 ssl;
        listen [::]:443 ssl;
        http2 on;
        server_name %(names)s;
        ssl_certificate     /etc/nginx/certs/%(safe)s.pem;
        ssl_certificate_key /etc/nginx/certs/%(safe)s.key;
        ssl_protocols       TLSv1.2 TLSv1.3;
        client_max_body_size 256m;
        location / {
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
        }
    }""" % {"names": names, "safe": safe})
os.makedirs("tls", exist_ok=True)
open("tls/nginx.conf", "w", encoding="utf-8").write(tpl_head + "".join(servers) + "\n}\n")
print("wrote tls/nginx.conf", len(blocks))
PY
COMPOSE='docker compose'
docker compose version >/dev/null 2>&1 || COMPOSE='docker-compose'
$COMPOSE up -d --force-recreate --no-deps nerve-tls
sleep 4
docker ps --filter name=nerve-prod-tls --format '{{.Status}}'
docker logs nerve-prod-tls --tail 8 2>&1 || true
curl -sS -o /dev/null -w 'www %{http_code}\n' --max-time 15 https://www.ahatec.com.cn/ || true
curl -sS -o /dev/null -w 'waf %{http_code}\n' --max-time 15 https://waf.ahatec.com.cn/api/health || true
curl -sSI --max-time 15 https://www.ahatec.com.cn/ | grep -iE '^(HTTP/|server:)' || true
