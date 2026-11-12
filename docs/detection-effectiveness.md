# 检出效果

本文说明 Nerve WAF **能拦什么、检出率如何对比**。与 [能做什么](./capabilities.md) 互补。

## 标准版 vs 专业版

| | **标准版** | **专业版（Arc-Cortex 引擎）** |
|---|------------|--------------------------------|
| **检测链** | CRS / Core | 上列 + Arc-Cortex Expert |
| **构建** | 默认 | `--features expert` + Antibody pack |
| **检出** | CRS 基线，建议实测 | **≥99%**（GoTestWAF），误报 **≤1%** |
| **适用** | PoC、规则以 CRS 为主 | 生产、深检与 benchmark |

## 防护覆盖（两档差异）

| 类别 | 标准版 | 专业版（Arc-Cortex 引擎） |
|------|--------|---------------------------|
| OWASP Top 10 | CRS 基线 | 更高检出，含编码绕过与 API 深检 |
| Bot / CC / Geo | 支持 | 支持 |
| AI 注入（可选） | 可选模块 | 可选 |

完整攻击类型列表见 [什么是 Nerve WAF · 防护哪些网络攻击](./what-is-waf.md)。

## 如何验证

装后快速验证：[快速入门](./getting-started.md) §检测验证。

专业版 benchmark（Antibody 仓库）：

```bash
cd Nerve-antibody
cargo run --release -p nerve-antibody-bench -- --help
```

完整 goTestWAF 基线编排脚本属于内部运维目录，不随开源仓库分发。

## 提升检出 / 降低误报

- 升级 **专业版**（启用 Expert + Arc-Cortex pack）
- 策略预演：`POST /api/policy/simulate`
- Console 误报标注 → allow 规则

## 能力边界

L3/L4 清洗 → DDoS；Ingress 路由 → Gateway。详见 [产品边界](./concepts/boundaries.md)。

## 下一步

| 目的 | 文档 |
|------|------|
| 延迟 | [性能与延迟](./performance.md) |
| 能力清单 | [能做什么](./capabilities.md) |
