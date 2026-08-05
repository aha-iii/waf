# blazehttp（长亭 WAF 拦截率测试工具）

本目录为指向本机 WSL 中克隆的软链接（避免把约 3 万条样本拷到 `/mnt/d` 极慢）。

## 获取 / 更新

```bash
# WSL
bash Nerve-waf-expert/scripts/fetch-blazehttp.sh
# 或
bash Nerve-waf/scripts/fetch-blazehttp.sh
```

默认克隆到 `/tmp/blazehttp`，并链到：

- `Nerve-waf-expert/vendor/blazehttp`
- `Nerve-waf/vendor/blazehttp`

上游：https://github.com/chaitin/blazehttp

## 测 Expert 拦截率

```bash
# 终端 1：Expert HTTP 适配（block→403 / 放行→200）
cd Nerve-waf-expert
cargo run -p nerve-waf-expert-bench
# LISTEN=0.0.0.0:8808 LEVEL=medium PACK=packs/baseline

# 终端 2
/tmp/blazehttp/blazehttp -t http://127.0.0.1:8808 -c 20
```
