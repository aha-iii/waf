# Nerve WAF 能做什么

本文按 **用户可感知的能力** 组织，而非按 crate 名称。

**安全效果与性能**（检出率、延迟、QPS）有独立专文，建议优先阅读：

| 主题 | 文档 |
|------|------|
| 能拦什么、检出率基线与验证 | [检出效果](./detection-effectiveness.md) |
| 延迟档位、吞吐与压测 | [性能与延迟](./performance.md) |
| 请求怎么走 | [怎么工作](./how-it-works.md) |

## 检测与防护

| 能力 | 说明 |
|------|------|
| **OWASP / CRS 规则** | 社区与基线规则包，Console 按模块启用 |
| **自定义规则** | 签名规则、Lua 插件段（pipeline `custom`） |
| **Expert / Arc-Cortex** | 可选 `--features expert`，Nerve-antibody 引擎（L0–L3 + Fusion） |
| **Core 语义增强** | libinjection 等 core 段，与 expert 流水线短路 |
| **CC 限流** | 连接/请求速率，模块 `nerve-waf-cc` |
| **Geo 准入** | 地域策略 |
| **Bot 检测** | Bot 模块准入 |
| **Challenge** | 挑战页 / 验证流 |
| **API 安全** | OpenAPI 路径/schema 校验（api-sec） |
| **AI 防护** | Prompt 注入等 AI 场景规则；可配置只观测 |
| **响应防篡改** | tamper 模块（hash） |
| **IP 封禁 / 情报** | intel 模块、与节点 Agent 可选联动 |

检测流水线默认顺序：`pre_access → expert → core → custom`（任一段命中即短路）。详见 [怎么工作](./how-it-works.md)。

## 策略与治理

| 能力 | 说明 |
|------|------|
| **站点 / 应用模型** | 按站点绑定模块与规则 |
| **快照下发** | Controller 生成 `WafEdgeSnapshot`，Agent watch 拉取 |
| **策略预演** | `POST /api/policy/simulate`，变更前模拟 |
| **RBAC / 多租户** | 控制面 RBAC |
| **License / 计量** | license 模块 |
| **CRS 包运维** | rules-ops 灰度 merge |

## 可观测与研判

| 能力 | 说明 |
|------|------|
| **攻击日志** | 事件持久化、Console 检索 |
| **多信号研判** | triage API + Console 详情 |
| **LLM / 模板解释** | explain API |
| **误报 Copilot** | fp-suggestions + 人工 confirm |
| **防护巡检** | posture scan |
| **Prometheus 指标** | 统一 metrics 面（规划中） |

日志与健康探针见 [observability.md](./observability.md)。

## 智能化（Assist，不替代定罪）

| 能力 | 说明 |
|------|------|
| **Nerve RIS** | 风险分层选 Fast/Deep 路径（Antibody） |
| **Shadow / 沙箱** | 候选规则旁路观察；晋升须人工确认 |
| **Engram 模型** | Nerve-llm Gateway assist；shadow→canary→prod |
| **MCP 工具** | Nerve-ai mcp-waf 对接 API |

平台级智能化地图：[Nerve-waf/docs/ai-capability-map.md](../../docs/ai-capability-map.md)

## 部署与集成

| 能力 | 说明 |
|------|------|
| **Docker Compose 试用** | `deploy/scripts/up.sh` |
| **Helm / K8s** | Controller + Agent Chart |
| **HA 多副本 Controller** | 无 sticky；Agent 用最后快照 |
| **Gateway 插件** | nerve-waf 插件 edge/remote |
| **HTTP 旁路 inspect** | `POST /api/inspect` |
| **Nerve-alert** | 可选告警 dispatch |

## 能力边界速查

```text
WAF 负责：L7 检测 · 策略 API · Agent 数据面 · WAF Console
WAF 不负责：L3/L4 清洗(DDoS) · API 入口路由(Gateway) · K8s 拓扑(Platform)
```

完整非目标：[concepts/boundaries.md](./concepts/boundaries.md)

## 下一步

| 目的 | 文档 |
|------|------|
| 检出率 / 误报 | [检出效果](./detection-effectiveness.md) |
| 延迟 / QPS | [性能与延迟](./performance.md) |
| 原理与数据流 | [怎么工作](./how-it-works.md) |
| 安装 | [installation.md](./installation.md) |
| API | [api.md](./api.md) |
| 生产 | [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md) |
