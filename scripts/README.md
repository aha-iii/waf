# Scripts — 测试 / 正式上传

本目录**只保留**打包装发与上传相关脚本。

## 测试环境（mars-test）

```bash
cp scripts/deploy-runtime.local.env.example scripts/deploy-runtime.local.env
# 编辑主机 / 账号 / 路径

bash scripts/deploy-test.sh              # 编译 + 打包 + 上传 + up
bash scripts/deploy-test.sh --skip-build # 跳过编译
# 或：
bash scripts/deploy-runtime-to-test.sh --up
```

Windows：`deploy-runtime-to-test.ps1` / `deploy-runtime-to-test-win.ps1`

## 正式环境

```bash
cp scripts/deploy-prod.local.env.example scripts/deploy-prod.local.env

bash scripts/deploy-prod-infra.sh        # 基础设施（首次 / 路由）
bash scripts/deploy-prod.sh --up         # 业务栈上传 + up
bash scripts/deploy-prod.sh --skip-build --up
```

## 打包（上传前）

```bash
bash scripts/pack-dist-runtime.sh --build       # 正式包
bash scripts/pack-dist-runtime.sh --build --test # 测试包（过滤部分规则）
```

## 本地密钥

`*.local.env` **不入库**。仅提交 `*.local.env.example`。

## `lib/`

被 pack / deploy 引用的公共函数，勿单独当入口跑。
