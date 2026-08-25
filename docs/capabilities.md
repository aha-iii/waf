# Nerve WAF 能做什么

本文按 **用户可感知的能力** 组织，而非按 crate 名称。实现状态标注为 **已交付** / **MVP** / **规划**，便于区分现状与路线图。

## 检测与防护

| 能力 | 说明 | 状态 |
|------|------|------|
| **OWASP / CRS 规则** | 社区与基线规则包，Console 按模块启用 | 已交付 |
| **自定义规则** | 签名规则、Lua 插件段（pipeline `custom`） | 已交付 |
| **Expert / Arc-Cortex** | 可选 `--features expert`，Nerve-antibody 引擎（L0–L3 + Fusion） | MVP |
| **Core 语义增强** | libinjection 等 core 段，与 expert 流水线短路 | 已交付 |
| **CC 限流** | 连接/请求速率，模块 `nerve-waf-cc` | 已交付 |
| **Geo 准入** | 地域策略 | 已交付 |
| **Bot 检测** | Bot 模块准入 | 已交付 |
| **Challenge** | 挑战页 / 验证流 | 已交付 |
| **API 安全** | OpenAPI 路径/schema 校验（api-sec） | MVP |
| **AI 防护** | Prompt 注入等 AI 场景规则；可配置只观测 | MVP |
| **响应防篡改** | tamper 模块（hash） | MVP |
| **IP 封禁 / 情报** | intel 模块、与节点 Agent 可选联动 | MVP |

检测流水线默认顺序：`pre_access → expert → core → custom`（任一段命中即短路）。详见 [怎么工作](./how-it-works.md)。

## 策略与治理

| 能力 | 说明 | 状态 |
|------|------|------|
| **站点 / 应用模型** | 按站点绑定模块与规则 | 已交付 |
| **快照下发** | Controller 生成 `WafEdgeSnapshot`，Agent watch 拉取 | 已交付 |
| **策略预演** | `POST /api/policy/simulate`，变更前模拟 | 已交付 |
| **RBAC / 多租户** | 控制面 RBAC crate | MVP |
| **License / 计量** | license crate | MVP |
| **CRS 包运维** | rules-ops 灰度 merge | MVP |

## 可观测与研判

| 能力 | 说明 | 状态 |
|------|------|------|
| **攻击日志** | 事件持久化、Console 检索 | 已交付 |
| **多信号研判** | triage API + Console 详情 | MVP |
| **LLM / 模板解释** | explain API | MVP |
| **误报 Copilot** | fp-suggestions + 人工 confirm | MVP |
| **防护巡检** | posture scan | MVP |
| **Prometheus 指标** | 规划统一 metrics 面 | 规划 |

日志与健康探针见 [observability.md](./observability.md)。

## 智能化（Assist，不替代定罪）

| 能力 | 说明 | 状态 |
|------|------|------|
| **Nerve RIS** | 风险分层选 Fast/Deep 路径（Antibody） | 已交付 |
| **Shadow / 沙箱** | 候选规则旁路观察；**晋升须人工** | 已定稿 |
| **Engram 模型** | Nerve-llm Gateway assist；shadow→canary→prod | MVP |
| **MCP 工具** | Nerve-ai mcp-waf 对接 API | MVP |

平台级智能化地图：[Nerve-waf/docs/ai-capability-map.md](../../docs/ai-capability-map.md)

## 部署与集成

| 能力 | 说明 | 状态 |
|------|------|------|
| **Docker Compose 试用** | `deploy/scripts/up.sh` | 已交付 |
| **Helm / K8s** | Controller + Agent Chart | 已交付 |
| **HA 多副本 Controller** | 无 sticky；Agent 用最后快照 | 已交付 |
| **Gateway 插件** | nerve-waf 插件 edge/remote | MVP |
| **HTTP 旁路 inspect** | `POST /api/inspect` | 已交付 |
| **Nerve-alert** | 可选告警 dispatch | MVP |

## 能力边界速查

```text
WAF 负责：L7 检测 · 策略 API · Agent 数据面 · WAF Console
WAF 不负责：L3/L4 清洗(DDoS) · API 入口路由(Gateway) · K8s 拓扑(Platform)
```

完整非目标：[concepts/boundaries.md](./concepts/boundaries.md)

## 下一步

| 目的 | 文档 |
|------|------|
| 原理与数据流 | [怎么工作](./how-it-works.md) |
| 安装 | [installation.md](./installation.md) |
| API | [api.md](./api.md) |
| 生产 | [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md) |
