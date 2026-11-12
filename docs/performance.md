# 性能与延迟

本文说明 Nerve WAF **额外引入多少延迟、吞吐如何扩展**。检出效果见 [检出效果](./detection-effectiveness.md)。

## 标准版 vs 专业版

| | **标准版** | **专业版（Arc-Cortex 引擎）** |
|---|------------|--------------------------------|
| **检测链** | `pre_access → core → custom` | `pre_access → expert → core → custom` |
| **构建** | 默认镜像 | `--features expert` + Antibody 规则包 |
| **典型场景** | PoC、CRS 为主、链路尽量短 | 生产、更高检出 |
| **检测延迟（P50）** | **≤35ms** | **≤20ms** |
| **全链路 P95** | **≤120ms** | **≤200ms** |

公共部分（Ingress、Agent 路由、反代、异步写库）两档一致。

## 全链路延迟拆解

| 模块 | 标准版 | 专业版（Arc-Cortex 引擎） | 说明 |
|------|--------|---------------------------|------|
| 入口转发（Ingress/LB） | ≤5ms | ≤5ms | TLS 终止通常在前面 |
| Agent 路由匹配 | ≤3ms | ≤3ms | 内存快照 |
| **L7 检测** | **≤35ms** | **≤20ms** | 标准版 CRS/Core；专业版 Arc-Cortex 引擎 |
| 事件写库 | 异步 | 异步 | 不占请求热路径 |
| Agent→上游反代 | ≤10ms | ≤10ms | RTT 另计 |

**全链路额外延迟（相对直连上游）：** 标准版 **+25～60ms**；专业版 **+15～50ms**。

## 吞吐与扩展

| 项 | 标准版 | 专业版（Arc-Cortex 引擎） |
|----|--------|---------------------------|
| Agent 水平扩展 | 按 QPS 扩副本 / DaemonSet | 同左 |
| 单核 QPS | ≥3000（待环境封板） | ≥2000（待环境封板） |

扩缩细节：[HA 部署模型](./ha-deployment.md)。

## 压测

公开入口是 Antibody 的 benchmark 二进制：

```bash
cd Nerve-antibody
cargo run --release -p nerve-antibody-bench -- --help
```

更完整的压测编排脚本（conc80 / gotestwaf / blazehttp 报表）属于内部运维目录，不随开源仓库分发。

标准版关闭 Expert；专业版开启 Expert 后对比。bench 常用 `deep` 模式，**勿与生产延迟直接对比**。

## 下一步

| 目的 | 文档 |
|------|------|
| 检出效果 | [检出效果](./detection-effectiveness.md) |
| 配置项 | [configuration.md](./configuration.md) |
