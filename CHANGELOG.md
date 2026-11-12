# Changelog

All notable changes to **Nerve WAF**（`Nerve-waf/Nerve-waf`）are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> 发布节奏：本仓库独立发版，版本号与 crate 版本（`Cargo.toml` 的 `workspace.package.version`）保持一致。
> 检测引擎子产品 Nerve Antibody 独立发版，版本对照见各版本「配套版本」小节。

## [Unreleased]

### Fixed

- **Console 全部 `/api/*` 返回 502（`waf-control unreachable`）**：`ops/mars-test/docker-compose.yml`
  与 `ops/prod/app/docker-compose.yml` 设置的是 `WAF_API_UPSTREAM`，而 `nerve-waf-console` 实际只读
  `WAF_CONTROLLER_UPSTREAM`。变量名不匹配 → 回落到默认值 `http://127.0.0.1:9000`；console 与
  controller 是两个独立容器，console 容器内的 `127.0.0.1:9000` 并无监听，于是代理层返回
  `{"error":"waf-control unreachable"}`。两处编排统一改为 `WAF_CONTROLLER_UPSTREAM`，
  并在 `console.rs` 增加对旧变量名的兼容回退，避免既有环境升级后再次静默回落。
  判据：`curl /api/sites` 由 `502` 变为 `401`（未带登录态），`/api/health` 为 `200`。

- **文档断链与占位符**：`docs/NAMING.md` 写 `./docs/PORTS.md`（应为 `./PORTS.md`）、
  `docs/releases/v1.0.0.md` 写 `../CHANGELOG.md`（应为 `../../CHANGELOG.md`），均差一层；
  `docs/deployment-modes.md`、`docs/what-is-waf.md` 使用 `/waf/`、`/ddos/` 站点绝对路由，
  在仓库视图下全部 404，改为相对路径或纯文本；`plugins/gateway/README.md` 指向本仓库不存在的
  同级目录；README 的 CI 徽章注释残留 `YOUR-ORG/YOUR-REPO` 占位，且托管平台是 Gitea 而非 GitHub。
  修正后全仓 223 条本地链接全部可达。

## [1.0.0] - 2026-11-12

首个正式发布版本。相比 `develop/1.0` 分支此前状态，本次补齐了开源合规、CI 与一键安装链路，
并修复了导致仓库无法编译、clone 后无法安装的两类阻塞问题。

### Added

**一键安装与运维**

- `deploy/install.sh` — 一键安装入口（`823b1cc`）：依赖自检 → 生成 `.env` → 构建启动 → 健康轮询 → 冒烟验证。
  支持 `--dry-run`、`--port`、`--agent-port`、`--uninstall`、`--purge`、`--force-env`、`--verbose`、`--version`；
  采用分级退出码，失败时自动打印相关容器日志，便于定位。
- `deploy/install.ps1` — Windows 侧入口，转发至 WSL 内执行 `install.sh`。
- `deploy/scripts/up.sh` / `up.ps1` — Compose 启动封装，供已有环境的二次启动。
- `scripts/start-waf.sh` / `start-waf.cmd` / `start-waf.ps1` — 源码三进程（controller + agent + console）调试启动。
- `scripts/wsl-port-forward.cmd` / `.ps1` — WSL2 端口转发，解决 Windows 下容器端口不可达。
- `scripts/smoke-test.sh` — 安装后验证脚本，6 项真实检查：Controller 健康、站点存在、策略绑定、边缘检测生效、
  事件落库、Agent 快照同步；自动发现站点 id，支持 `--json` 供 CI 消费。

**CI 与工程化**

- `.github/workflows/nerve-waf-ci.yml` — `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`、`helm lint` 四道门禁（`823b1cc`）。

**文档**

- `docs/performance.md` — 性能基线与调优参数说明。
- `docs/detection-effectiveness.md` — 检测有效性评估方法与样本口径。
- `docs/deployment-modes.md` — 单机 / Compose / K8s 三种部署形态的取舍与拓扑。

**测试**

- `crates/contract/nerve-waf-dataplane/tests/parity.rs` — 边缘 dataplane 与内嵌引擎的判定一致性（integration test），
  替代原 `src/test/test_edge_dataplane_parity.rs`（`1ff4bf3`）。

### Changed

- **`.gitignore` 由黑名单改为白名单**（`823b1cc`）：`scripts/`、`deploy/scripts/` 下的脚本此前被忽略规则吞掉，
  从未真正入库——这是「clone 后按文档执行却找不到脚本」的直接原因，本次一并放行。
- **`.gitattributes` 统一 EOL**（`823b1cc`）：`*.sh`/`*.rs`/`*.md` 等强制 LF，`*.cmd`/`*.ps1` 强制 CRLF，
  消除 Windows 与 Linux 间往返产生的整文件 diff。
- **全仓 `cargo fmt` 规范化**（`0325e84`）：133 个文件，仅换行、缩进与 import 排序变化，无语义变更；
  此后由 CI 强制校验。
- `scripts/smoke-test.sh` 重写：由「只看端口是否可连」改为端到端功能验证（见上）。
- `docs/installation.md`、`deploy/README.md`、`deploy/START.md`、`deploy/TOPOLOGY.md` —
  修正失效路径、过期端口与错误的 Compose 文件名。
- `docs/DEPENDENCIES.md`、`docs/NAMING.md`、`docs/PORTS.md`、`docs/capabilities.md`、
  `docs/ha-deployment.md`、`docs/what-is-waf.md` — 与当前实现对齐。
- `README.md` — 补充 License 段、修复断链、重写 Dashboard 说明。

### Fixed

- **仓库无法编译**（`1ff4bf3`）：`crates/contract/nerve-waf-dataplane/src/test/test_snapshot_contract.rs` 中
  `fn` 被误写为 `fe`、类型 `WafEdgeSnapshot` 被误写为 `WafEdgeSeapshot`、测试名 `..._aed_global` 拼写错误，
  导致 `cargo test` 在解析阶段即失败。
- **rustfmt 无法运行**（`1ff4bf3`）：`vendor/proxy-wasm-test-framework` 示例中 `"\e"` 是非法转义，
  rustfmt 1.8.0 起报 `unknown character escape`，改为 `"\x1b"`（语义等价）。
- **端口占用时只输出 panic + backtrace**（`1ff4bf3`）：Controller 启动时 bind/serve 失败改为按
  `AddrInUse` / `PermissionDenied` 分类输出可执行提示（换端口参数、查占用命令）并以 `exit(1)` 退出。
- **SQL 注释规则误报过高**（`1ff4bf3`）：`sql-003` 由 `(?i)(--|#|/\*)` 收紧为 `(?i)(\s--\s|['"]\s*--|/\*)`，
  不再把 URL 锚点 `#`、路径中的裸 `--` 判为注入；块注释与「引号尾 `--`」两种真实形态保留。
  同步更新 `apps/console` 规则说明与 `plugins/l7/owasp-sqli` 副本。
- **parity 测试类型分裂**（`1ff4bf3`）：原测试放在 `src/test/` 下，`cfg(test)` 编译出的 lib 与
  `nerve_waf_embed` 链接的 lib 是两份类型，断言无法通过；迁至 `tests/parity.rs` 后统一。
- **空 submodule 导致 clone 失败**（`823b1cc`）：`vendor/coreruleset` 从 git 索引移除（文件系统内保留），
  不再要求 clone 时初始化该子模块。

### Breaking

- **风险分类返回值改名**：`insufficinet_data` → `insufficient_data`（`1ff4bf3`）。
  该字符串出现在 API 响应与测试断言中；若有外部系统按此字符串做判断，升级后需同步修改。
- **`sql-003` 判定范围收紧**：仅命中 `#` 或裸 `--` 的历史告警此后不再产生，看板与告警统计会出现台阶式下降。

### 配套版本

| 仓库 | 版本 | 说明 |
|------|------|------|
| Nerve WAF | `v1.0.0` | 本仓库 |
| Nerve Antibody | `v0.1.0` | 可选 `expert` feature 依赖，需与 `Nerve-waf` 目录同级 |

`nerve-waf-inspect` 通过相对路径依赖 `../../../../Nerve-antibody/crates/integration/nerve-antibody`（optional `expert`）。
如需启用专家引擎，两个仓库必须保持如下同级布局：

```text
<workspace>/
├── Nerve-waf/          # 本仓库
└── Nerve-antibody/     # 检测引擎
```

### 提交明细

| Commit | 类型 | 说明 |
|--------|------|------|
| `823b1cc` | chore | 补齐开源合规、CI 与安装部署工具链（33 文件，+1078/−116） |
| `0325e84` | style | 全仓 cargo fmt 规范化（133 文件，+2327/−2092，无语义变更） |
| `1ff4bf3` | fix | 修复 HEAD 无法编译的缺陷并统一错误码拼写（81 文件，+1224/−1115） |

此前历史（作为 1.0.0 的代码基线，未单独发版）：

| Commit | 说明 |
|--------|------|
| `e1c72b3` | 模块名修改 |
| `28ee6ef` | 注释调整 |
| `195d2da` | Stop tracking local deploy scripts |
| `be9e9e4` | 删除无用脚本 |
| `59657fe` | Nerve-defender 改名 Nerve-waf |
| `628bc4e` | feat: 优化规则算法 |
| `d79b051` | Initial commit: Nerve-defender |

[Unreleased]: https://git.ahatec.com.cn/nerve/nerve-waf/nerve-defender/-/compare/v1.0.0...develop/1.0
[1.0.0]: https://git.ahatec.com.cn/nerve/nerve-waf/nerve-defender/-/tags/v1.0.0
