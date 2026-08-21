-- Sample Lua L7 plugin: custom Header / URI checks (inspect returns nil or hit table).
-- Sample custom Header / URI checks

function inspect(req)
  local headers = req.headers or ""

  for line in string.gmatch(headers, "[^\r\n]+") do
    local name, value = string.match(line, "^([^:%s]+)%s*:%s*(.*)$")
    if name and value then
      local n = string.lower(name)
      if n == "x-forwarded-for" and #value > 100 then
        return {
          blocked = true,
          action = "block",
          category = "custom",
          rule_id = "lua-hdr-xff-len",
          rule_name = "XFF 过长",
          severity = "medium",
          matched_field = "headers",
          payload_sample = string.sub(line, 1, 200),
        }
      end
      if n == "user-agent" and value == "" then
        return {
          blocked = false,
          action = "log",
          category = "custom",
          rule_id = "lua-hdr-empty-ua",
          rule_name = "空 User-Agent",
          severity = "low",
          matched_field = "headers",
          payload_sample = line,
        }
      end
    end
  end

  if string.find(string.lower(req.uri or ""), "sqlmap", 1, true) then
    return {
      blocked = true,
      action = "block",
      category = "custom",
      rule_id = "lua-uri-sqlmap",
      rule_name = "URI 含 sqlmap",
      severity = "high",
      matched_field = "uri",
      payload_sample = req.uri,
    }
  end

  return nil
end
