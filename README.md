# Nerve WAF

**Nerve WAF**（路径 `Nerve-waf/Nerve-waf`）：L7 检测流水线、控制面 API、WAF Console、边缘数据面。  
Crate 前缀 **`nerve-waf-*`**（见 [docs/NAMING.md](./docs/NAMING.md)）。

| 组件 | 默认 | 说明 |
|------|------|------|
| Control API | `:8781` | `nerve-waf-api` |
| Console | `:5174` / 部署后见 compose | `apps/console-react` |
| Nerve Agent（可选） | `:8766` | 生态仓 `Nerve/` |
| Nerve-alert（可选） | `:8790` | 同级仓 `Nerve-alert/` |

文档：[DEPENDENCIES](./docs/DEPENDENCIES.md) · [ARCHITECTURE](./docs/ARCHITECTURE.md) · [CONTRIBUTING](./CONTRIBUTING.md) · [SECURITY](./SECURITY.md)

## 本地开发

```bash
cp .env.example .env
cargo build --release -p nerve-waf-api -p nerve-waf-edge
NERVE_WAF_DATA=./data cargo run -p nerve-waf-api --release   # :8781
cd apps/console-react && npm install && npm run dev          # Vite
```

同级依赖：`bash scripts/` 已精简为发版脚本；布局见 `docs/DEPENDENCIES.md`。

## 测试 / 正式上传

详见 [scripts/README.md](./scripts/README.md)。

```bash
# 测试
cp scripts/deploy-runtime.local.env.example scripts/deploy-runtime.local.env
bash scripts/deploy-test.sh

# 正式
cp scripts/deploy-prod.local.env.example scripts/deploy-prod.local.env
bash scripts/deploy-prod-infra.sh
bash scripts/deploy-prod.sh --up
```

## Workspace 分层

```text
crates/{contract,engine,modules,controlplane,dataplane}/
apps/console-react/
scripts/          # 仅测试/正式打包上传
deploy/prod/      # 正式 compose / infra
```

## 许可证

[MIT](./LICENSE)
