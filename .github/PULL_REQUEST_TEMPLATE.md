## 这个 PR 做了什么

<!-- 一句话说明动机；关联 issue 用 #123 -->

## 类型

- [ ] 控制面 / 数据面
- [ ] 检测引擎 / 规则
- [ ] 策略与下发（policy / snapshot）
- [ ] Console / 部署（helm、compose）
- [ ] 文档 / 构建

## 自检（与 CONTRIBUTING 的 quality bar 一致）

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] 规则 / 策略改动已附样本，并说明误报漏报影响
- [ ] 文档中的命令与链接我实际跑过 / 点过

## 行为影响

<!-- 是否改变检测口径、时延或内存占用？有数据更好（bench / 误报漏报变化） -->

## 备注

<!-- 已知限制、后续计划、需要 reviewer 特别关注的地方 -->
