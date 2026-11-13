---
name: Bug report
about: 组件崩溃、构建失败或请求处理异常
title: "[bug] "
labels: bug
---

## 现象

<!-- 发生了什么，期望是什么。涉及拦截/放行判断差异的，请贴原始请求 -->

## 复现

```bash
# 最小复现命令（helm install / docker compose / 直接跑二进制）
```

请求（如涉及检测判断）：

```http
GET /path?x=1 HTTP/1.1
Host: example.com
```

## 环境

| 项 | 值 |
|---|---|
| commit / tag | |
| 组件 | controller / agent / console |
| 部署方式 | helm / docker compose / 裸机 |
| CRS 版本 | |
| OS + Rust 版本 | |
| 相关 feature | 无 / `expert` / `accel-hyperscan` |

## 日志 / 报错

```
粘贴 panic backtrace 或关键日志；长日志请附文件
```

## 已排除

<!-- 例如：换规则包仍复现、关掉 Expert 引擎仍复现、绕过 agent 直连 controller 仍复现 -->
