# 什么是 Nerve WAF

Nerve WAF 是 **L7 Web 应用防火墙**：在 HTTP/S 流量到达业务之前，完成准入、检测与裁决（放行 / 挑战 / 拦截），并通过控制面统一管理策略与站点。

## 防护哪些网络攻击

Nerve WAF 在 **HTTP/HTTPS 应用层（L7）** 识别并拦截常见 Web 攻击。下表按**攻击类型**说明防护范围；检出率与 benchmark 见 [检出效果](./detection-effectiveness.md)。

| 攻击类型 | 典型手法 | Nerve WAF 如何防护 |
|----------|----------|-------------------|
| **SQL 注入（SQLi）** | 在参数、Cookie、Header 中插入 SQL 语句 | CRS 规则 + 语义分析；union/select 等特征匹配 |
| **跨站脚本（XSS）** | 注入 `<script>`、事件处理器等可执行内容 | 反射型/存储型 payload 规则与语义检测 |
| **命令 / 代码注入（RCE）** | OS 命令、模板注入、表达式执行 | 规则签名 + Expert 深检（可疑请求升 Deep） |
| **路径遍历 / 文件包含** | `../`、`/etc/passwd`、编码绕过读敏感文件 | 路径规范化 + 目录穿越规则 |
| **SSRF** | 让服务端访问内网/metadata URL | URL/Host 特征规则与语义校验 |
| **XML / XXE** | 恶意 DTD、外部实体引用 | CRS 与自定义规则（视模块启用） |
| **HTTP 协议滥用** | 异常 Method、畸形 Header、走私尝试 | 协议校验与准入规则 |
| **API / JSON 注入** | JSON 字段内嵌 SQLi/XSS、异常 schema | JSON 解析 + Core/Expert 字段级检测 |
| **CC / HTTP 洪泛** | 单 IP/会话高频刷接口 | 准入层连接/请求速率限制（`pre_access`） |
| **恶意 Bot / 扫描器** | 批量扫漏洞、撞库、爬虫 | Bot 识别模块 + 可选 Challenge |
| **地域滥用** | 来自高风险地区的攻击流量 | Geo 准入策略（允许/拒绝/观察） |
| **AI 相关攻击** | Prompt 注入、越狱话术（可选） | AI 防护模块；可仅记录不拦截 |

**分层说明：**

- **L7 Web 攻击**（上表）→ 由 Nerve WAF 检测与裁决（拦截 / 挑战 / 放行）。
- **L3/L4 洪泛、SYN flood 等** → 由 [Nerve DDoS](/ddos/) 负责，不在 WAF 范围内。

编码绕过（双重 URL、Base64 等）在 decode 管线中解码后再匹配，详见 [怎么工作](./how-it-works.md)。

## 由哪些进程组成

WAF **不是单进程**，而是三个可独立扩缩的角色：

| 进程 | 角色 | 默认端口 | 说明 |
|------|------|----------|------|
| `nerve-waf-controller` | **控制面** | 9000 | API、策略 CRUD、快照生成、事件持久化 |
| `nerve-waf-agent` | **数据面** | 9080 | 接收流量、执行检测、反代上游 |
| `nerve-waf-console` | **管理 UI**（可选） | 9090 | 静态 Console + 反代 Controller |

```text
运维 / API ──► Controller ──► PostgreSQL（站点、规则、事件）
                    │
                    │ watch / dataplane 快照
                    ▼
客户端流量 ──► Agent ──► 上游业务
```

**最小可用栈：** Controller + Agent（+ PostgreSQL）。Console 可选；无 Console 时可用 API 或独立构建的 UI。

## 在 Nerve 平台中的位置

- WAF 负责 **L7 应用层检测**；L3/L4 清洗由 [Nerve DDoS](/ddos/) 负责（可选串联）。
- 生产常见路径：`Client → [DDoS] → WAF Agent → 上游`（DDoS 可选）。
- 平台总览：[Nerve 系统架构](../../../docs/SYSTEM-ARCHITECTURE.md) · 产品边界：[boundaries.md](./concepts/boundaries.md)

## 不是什么

| 非目标 | 说明 |
|--------|------|
| Ingress Controller | Agent 可接在 Ingress 之后，不实现 Ingress 本身 |
| L3/L4 清洗 | 由 DDoS 负责；WAF 不替代 |
| 硬依赖其它产品 | DDoS 等为可选 sibling |

## 下一步读什么

| 目的 | 文档 |
|------|------|
| 能力清单 | [能做什么](./capabilities.md) |
| 检出率 / 误报 | [检出效果](./detection-effectiveness.md) |
| 性能 / 延迟 | [性能与延迟](./performance.md) |
| 请求怎么走、策略怎么同步 | [怎么工作](./how-it-works.md) |
| 5 分钟试用 | [快速入门](./getting-started.md) · [deploy/START.md](../deploy/START.md) |
| 运行时架构细节 | [architecture-overview.md](./architecture-overview.md) |
