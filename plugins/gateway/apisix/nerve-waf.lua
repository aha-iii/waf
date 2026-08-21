--
-- APISIX plugin: HTTP bypass to Nerve WAF POST /api/inspect.
-- Agent eBPF L7 uses Unix socket; this path is independent and coexists.
-- Install: copy into apisix/plugins/, add "nerve-waf" to conf plugins list, reload.
-- Change waf_api / api_key placeholders before production use. See README.md.
-- APISIX plugin: HTTP bypass to Nerve WAF (coexists with Agent Unix socket path)
--

local core = require("apisix.core")
local http = require("resty.http")
local ngx = ngx

local plugin_name = "nerve-waf"

local schema = {
    type = "object",
    properties = {
        waf_api = {
            type = "string",
            default = "http://127.0.0.1:8780",
        },
        api_key = {
            type = "string",
        },
        app_id = {
            type = "string",
            default = "default",
        },
        block_on_hit = {
            type = "boolean",
            default = true,
        },
        fail_open = {
            type = "boolean",
            default = false,
        },
        persist_on_hit = {
            type = "boolean",
            default = true,
        },
        timeout = {
            type = "integer",
            minimum = 100,
            default = 2000,
        },
        block_status = {
            type = "integer",
            default = 403,
        },
        block_body = {
            type = "string",
            default = "Blocked by Nerve WAF",
        },
    },
}

local _M = {
    version = 0.1,
    priority = 2800,
    name = plugin_name,
    schema = schema,
}

local function trim_slash(s)
    if not s then
        return ""
    end
    return (s:gsub("/+$", ""))
end

local function headers_to_string(ctx)
    local headers = core.request.headers(ctx) or {}
    local lines = {}
    for name, value in pairs(headers) do
        if type(value) == "table" then
            for _, v in ipairs(value) do
                lines[#lines + 1] = name .. ": " .. tostring(v)
            end
        else
            lines[#lines + 1] = name .. ": " .. tostring(value)
        end
    end
    return table.concat(lines, "\n")
end

local function read_body(max_bytes)
    ngx.req.read_body()
    local body = ngx.req.get_body_data()
    if not body then
        local path = ngx.req.get_body_file()
        if path then
            local f = io.open(path, "rb")
            if f then
                body = f:read(max_bytes or 65536)
                f:close()
            end
        end
    end
    return body or ""
end

local function html_escape(s)
    if not s then
        return ""
    end
    return (tostring(s)
        :gsub("&", "&amp;")
        :gsub("<", "&lt;")
        :gsub(">", "&gt;")
        :gsub('"', "&quot;")
        :gsub("'", "&#39;"))
end

local function block_page_html(rule, subtitle)
    rule = html_escape(rule or "waf")
    subtitle = html_escape(subtitle or "您的请求触发了 Web 应用防火墙规则，已被安全策略拦截。")
    return [[<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>访问已被拦截 · Nerve WAF</title>
<style>
*{box-sizing:border-box;margin:0;padding:0}
body{
  min-height:100vh;display:flex;align-items:center;justify-content:center;
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;
  background:linear-gradient(145deg,#0f172a 0%,#1e293b 45%,#0f172a 100%);
  color:#e2e8f0;padding:24px
}
.card{
  width:100%;max-width:480px;background:rgba(255,255,255,.04);
  border:1px solid rgba(255,255,255,.08);border-radius:16px;
  padding:40px 36px 32px;text-align:center;
  box-shadow:0 24px 48px rgba(0,0,0,.35)
}
.icon{
  width:72px;height:72px;margin:0 auto 24px;
  background:linear-gradient(135deg,#ef4444,#dc2626);
  border-radius:50%;display:flex;align-items:center;justify-content:center;
  box-shadow:0 8px 24px rgba(239,68,68,.35)
}
.icon svg{width:36px;height:36px;fill:#fff}
.code{
  display:inline-block;font-size:13px;font-weight:600;letter-spacing:.08em;
  color:#f87171;background:rgba(239,68,68,.12);
  border:1px solid rgba(239,68,68,.25);border-radius:999px;
  padding:4px 14px;margin-bottom:16px
}
h1{font-size:22px;font-weight:600;color:#f8fafc;margin-bottom:12px;line-height:1.4}
.desc{font-size:14px;color:#94a3b8;line-height:1.7;margin-bottom:24px}
.rule{
  display:inline-flex;align-items:center;gap:6px;
  font-size:12px;color:#cbd5e1;background:rgba(255,255,255,.06);
  border:1px solid rgba(255,255,255,.1);border-radius:8px;
  padding:8px 14px;margin-bottom:28px;word-break:break-all
}
.rule-label{color:#64748b;flex-shrink:0}
.footer{
  font-size:12px;color:#475569;border-top:1px solid rgba(255,255,255,.06);
  padding-top:20px;display:flex;align-items:center;justify-content:center;gap:8px
}
.footer-dot{width:6px;height:6px;border-radius:50%;background:#22c55e}
</style>
</head>
<body>
<div class="card">
  <div class="icon">
    <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-1 14h2v2h-2v-2zm0-8h2v6h-2V7z"/></svg>
  </div>
  <div class="code">HTTP 403</div>
  <h1>访问已被拦截</h1>
  <p class="desc">]] .. subtitle .. [[</p>
  <div class="rule"><span class="rule-label">规则</span><span>]] .. rule .. [[</span></div>
  <div class="footer"><span class="footer-dot"></span>Nerve WAF · Web Application Firewall</div>
</div>
</body>
</html>]]
end

local function pick_rule_name(data)
    if not data then
        return "waf"
    end
    local hit = data.hit
    if hit and hit.rule_name then
        return hit.rule_name
    end
    if data.rule_name then
        return data.rule_name
    end
    return "waf"
end

local function header_ci(headers, ...)
    if not headers then
        return nil
    end
    for _, name in ipairs({...}) do
        local v = headers[name]
        if v and v ~= "" then
            return v
        end
        local lower = string.lower(name)
        for k, val in pairs(headers) do
            if type(k) == "string" and string.lower(k) == lower and val and val ~= "" then
                if type(val) == "table" then
                    return val[1]
                end
                return val
            end
        end
    end
    return nil
end

local function parse_traceparent(raw)
    if not raw or raw == "" then
        return nil, nil
    end
    local ver, trace_id, span_id = string.match(raw, "^(%x+)%-(%x+)%-(%x+)%-(%x+)$")
    if ver ~= "00" or not trace_id or not span_id then
        return nil, nil
    end
    if #trace_id ~= 32 or #span_id ~= 16 then
        return nil, nil
    end
    return trace_id, span_id
end

local function parse_tracestate(raw)
    if not raw or raw == "" then
        return nil, nil, nil
    end
    for part in string.gmatch(raw, "[^,]+") do
        part = part:match("^%s*(.-)%s*$")
        local inner = part:match("^nerve=(.+)$") or part:match("^Nerve=(.+)$")
        if inner then
            local journey_id, parent_journey, site_id
            for kv in string.gmatch(inner, "[^,]+") do
                kv = kv:match("^%s*(.-)%s*$")
                local k, v = kv:match("^([^:]+):(.+)$")
                if k and v then
                    k = k:match("^%s*(.-)%s*$")
                    v = v:match("^%s*(.-)%s*$")
                    if k == "journey" then
                        journey_id = v
                    elseif k == "parent" then
                        parent_journey = v
                    elseif k == "site" then
                        site_id = v
                    end
                end
            end
            return journey_id, parent_journey, site_id
        end
    end
    return nil, nil, nil
end

local function format_tracestate(journey_id, parent_journey, site_id)
    local parts = {}
    if journey_id and journey_id ~= "" then
        parts[#parts + 1] = "journey:" .. journey_id
    end
    if parent_journey and parent_journey ~= "" then
        parts[#parts + 1] = "parent:" .. parent_journey
    end
    if site_id and site_id ~= "" then
        parts[#parts + 1] = "site:" .. site_id
    end
    if #parts == 0 then
        return nil
    end
    return "nerve=" .. table.concat(parts, ",")
end

local function resolve_trace_context(req_headers)
    local tp = header_ci(req_headers, "traceparent", "Traceparent")
    local tp_trace, tp_span = parse_traceparent(tp)
    local ts_journey, ts_parent, ts_site = parse_tracestate(
        header_ci(req_headers, "tracestate", "Tracestate"))
    local trace_id = header_ci(req_headers,
        "Nerve-Trace-Id", "nerve-trace-id",
        "X-Nerve-Trace-Id", "x-nerve-trace-id",
        "X-Trace-Id", "x-trace-id")
    if not trace_id then
        trace_id = tp_trace
    end
    local span_id = header_ci(req_headers, "Nerve-Span-Id", "nerve-span-id") or tp_span
    local journey_id = header_ci(req_headers, "Nerve-Journey-Id", "nerve-journey-id") or ts_journey
    local parent_journey = header_ci(req_headers, "Nerve-Parent-Journey-Id", "nerve-parent-journey-id") or ts_parent
    local session_id = header_ci(req_headers, "Nerve-Session-Id", "nerve-session-id") or journey_id
    return trace_id, span_id, journey_id, parent_journey, session_id, ts_site, tp
end

local function set_upstream_trace(ctx, trace_id, span_id, journey_id, parent_journey, session_id, site_id, tp)
    if trace_id then
        core.request.set_header(ctx, "Nerve-Trace-Id", trace_id)
        core.request.set_header(ctx, "X-Nerve-Trace-Id", trace_id)
    end
    if span_id then
        core.request.set_header(ctx, "Nerve-Span-Id", span_id)
    end
    if journey_id then
        core.request.set_header(ctx, "Nerve-Journey-Id", journey_id)
    end
    if parent_journey then
        core.request.set_header(ctx, "Nerve-Parent-Journey-Id", parent_journey)
    end
    if session_id then
        core.request.set_header(ctx, "Nerve-Session-Id", session_id)
    end
    if tp then
        core.request.set_header(ctx, "traceparent", tp)
    elseif trace_id and span_id then
        core.request.set_header(ctx, "traceparent",
            string.format("00-%s-%s-01", trace_id, span_id))
    end
    local ts = format_tracestate(journey_id, parent_journey, site_id)
    if ts then
        core.request.set_header(ctx, "tracestate", ts)
    end
end

function _M.check_schema(conf)
    return core.schema.check(schema, conf)
end

function _M.access(conf, ctx)
    local waf_api = trim_slash(conf.waf_api or "http://127.0.0.1:8780")
    local url = waf_api .. "/api/inspect"
    local timeout = conf.timeout or 2000

    local req_headers = core.request.headers(ctx) or {}
    local trace_id, span_id, journey_id, parent_journey, session_id, site_id, tp =
        resolve_trace_context(req_headers)

    local inspect_body = {
        app_id = conf.app_id or "default",
        method = ngx.req.get_method(),
        host = ngx.var.host,
        uri = ngx.var.request_uri,
        headers = headers_to_string(ctx),
        body = read_body(65536),
        src_ip = core.request.get_remote_client_ip(ctx) or ngx.var.remote_addr,
        user_agent = req_headers["user-agent"] or "",
        persist = conf.persist_on_hit ~= false,
        trace_id = trace_id,
        journey_id = journey_id,
        session_id = session_id,
        parent_journey_id = parent_journey,
    }

    local httpc = http.new()
    httpc:set_timeout(timeout)

    local out_headers = {
        ["Content-Type"] = "application/json",
    }
    if conf.api_key and conf.api_key ~= "" then
        out_headers["x-api-key"] = conf.api_key
    end
    if trace_id then
        out_headers["Nerve-Trace-Id"] = trace_id
    end
    if span_id then
        out_headers["Nerve-Span-Id"] = span_id
    end
    if journey_id then
        out_headers["Nerve-Journey-Id"] = journey_id
    end
    if parent_journey then
        out_headers["Nerve-Parent-Journey-Id"] = parent_journey
    end
    if tp then
        out_headers["traceparent"] = tp
    end

    local res, err = httpc:request_uri(url, {
        method = "POST",
        body = core.json.encode(inspect_body),
        headers = out_headers,
        keepalive = true,
    })

    if not res then
        core.log.warn("nerve-waf inspect failed: ", err or "unknown")
        if conf.fail_open then
            return
        end
        return core.response.exit(503, conf.block_body or "WAF inspect unavailable")
    end

    if res.status == 401 then
        core.log.warn("nerve-waf: WAF returned 401, check api_key")
        if conf.fail_open then
            return
        end
        return core.response.exit(503, "WAF authentication failed")
    end

    if res.status < 200 or res.status >= 300 then
        core.log.warn("nerve-waf inspect http ", res.status, " body=", res.body or "")
        if conf.fail_open then
            return
        end
        return core.response.exit(503, "WAF inspect error: " .. tostring(res.status))
    end

    local data, decode_err = core.json.decode(res.body)
    if not data then
        core.log.error("nerve-waf invalid json: ", decode_err or "decode failed")
        if conf.fail_open then
            return
        end
        return core.response.exit(503, "WAF inspect invalid response")
    end

    if data.blocked and conf.block_on_hit ~= false then
        local rule = pick_rule_name(data)
        local html = block_page_html(rule, conf.block_body ~= "Blocked by Nerve WAF" and conf.block_body or nil)
        core.log.warn("nerve-waf blocked uri=", ngx.var.request_uri, " rule=", rule)
        ngx.header["Content-Type"] = "text/html; charset=utf-8"
        return core.response.exit(conf.block_status or 403, html)
    end

    -- Prefer WAF response trace (newly generated or normalized ID)
    local waf_trace = data.trace
    if waf_trace and type(waf_trace) == "table" then
        trace_id = waf_trace.trace_id or trace_id
        span_id = waf_trace.span_id or span_id
        journey_id = waf_trace.journey_id or journey_id
        parent_journey = waf_trace.parent_journey_id or parent_journey
        session_id = waf_trace.session_id or session_id
        site_id = waf_trace.site_id or site_id or conf.app_id
        tp = waf_trace.traceparent or tp
    end
    set_upstream_trace(ctx, trace_id, span_id, journey_id, parent_journey, session_id, site_id, tp)
end

return _M
