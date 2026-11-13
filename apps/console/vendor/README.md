# vendor/nerve-ui

`@nerve/ui` 的上游源码副本，供 `apps/console` 构建使用。Console 通过
`package.json` 的 `file:./vendor/nerve-ui`、Vite 的 `resolve.alias` 与
`tsconfig.json` 的 `paths` 三处指向本目录，直接读 TypeScript 源码，无需预构建该包。

| 项 | 值 |
|---|---|
| 上游 | `nerve/nerve-ui`（内网 Gitea 仓库，分支 `main`） |
| 提交 | `39efd9b` |
| 导入方式 | 按上游 `git ls-files` 清单逐文件复制（保留原始字节与行尾） |
| 排除项 | `.git/`、`node_modules/`、`dist/`、`package-lock.json` |
| 文件数 | 58 |

副本内容与上游该提交**逐文件 SHA-256 一致**，未做任何本地修改；因此后续可以用
`git ls-files` + 哈希比对来做同步校验。

`.gitattributes` 对本目录关闭了行尾转换（`nerve-ui/** -text`），否则仓库级的
`core.autocrlf` 会把副本改写成 LF，与上游不再逐字节一致。

## 授权

上游 `package.json` 标注 **`"license": "UNLICENSED"`**，`publishConfig.access` 为
`restricted`。副本按原样保留，未改动上游元数据。

**在确认授权之前，本目录不应随公开仓库发布**（或需先为 Nerve-ui 补一份
MIT / Apache-2.0 许可证）。
