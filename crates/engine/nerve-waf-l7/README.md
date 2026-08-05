# nerve-waf-l7

L7 HTTP 检测引擎内核。规则来源：

1. **外置**：`plugins/l7/*/rules/*.json`（plugin-host 加载，热更新）
2. **回退**：`rules/*.json` 内置 embed（plugins 目录为空时）

## API

```rust
let engine = L7Engine::from_rules(modules, compiled_rules);
engine.inspect(&InspectRequest { uri: "/?id=1 or 1=1".into(), ..Default::default() });
```

## 规则格式

见 `plugins/l7/README.md`。

## 编译

```bash
cargo test -p nerve-waf-l7
cargo build -p nerve-waf-l7
```

运行时由 Control API 通过 plugin-host 组装，一般无需单独运行。
