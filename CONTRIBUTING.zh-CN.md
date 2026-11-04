# 参与 specX 贡献

specX 是一个服务目录与规范工作台。贡献应保持服务目录、授权边界、持久化、Web Host 和 Docker 部署之间的关系清晰。

## 开发环境

需要 Node.js 22+、pnpm 10.34.7、Rust 1.90+ 和 Make。

```sh
pnpm install
pnpm dev
```

需要验证真实持久化时，使用本地密码启动 API：

```sh
SPECX_PASSWORD='choose-a-password-at-least-12-chars' pnpm dev:backend
```

提交前运行 `pnpm build:checked` 和 `pnpm test:specx`。修改 Docker 部署时，在环境允许的情况下额外运行 `pnpm docker:build`。

## 项目边界

| 路径 | 用途 |
| --- | --- |
| `apps/desktop/` | Web 与 Tauri 共用的 Vue 3 + Pinia 前端 |
| `src-tauri/` | 原生桌面壳和 IPC 适配层，有意独立于 `crates/` |
| `core/` | 服务、目录、标签、规范、授权和持久化领域 |
| `crates/specx-api/` | HTTP 资源和应用服务边界 |
| `crates/specx-web/` | 密码会话、请求上下文、静态文件和 API 挂载 |
| `deploy/` | Docker 镜像和 Compose 部署 |
| `docs/` | 当前架构和 API 说明 |

`core/` 中的持久化适配器只负责 specX 自身存储；外部管理工具和扩展运行时不属于当前基线。

Web 和桌面必须调用同一个 `specx-api` router。传输差异放在
`apps/desktop/src/lib/backend`，目录业务规则放在 `core/` 或
`crates/specx-api`，不得在桌面命令或前端组件中复制一份。

## 修改约定

将改动放在负责该行为的层中。API 写入必须保留租户/工作区授权和并发控制；浏览器存储不可用时，前端仍应能浏览服务目录。新的部署配置统一使用 `SPECX_` 前缀，并在 Docker 部署说明中记录。
