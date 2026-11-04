# 基础能力验收记录

本文件记录共享应用壳、Web Host、桌面 Host 与发布基础能力的验证范围。记录中的
结果对应执行日期；持续集成结果是后续提交的权威状态。最后更新：2026-11-05。

## 验收范围

| 区域 | 验证方式 | 结果与边界 |
| --- | --- | --- |
| 前端 | Vue 类型检查、生产构建、Vitest、格式与开发 CSS 检查 | 主题、语言、快捷键、菜单、设置、目录和提示行为通过；开发 CSS 有独立防回归检查 |
| API/Core | Rust workspace 格式、检查与测试 | SQLite 持久化、授权、资源并发、幂等与错误契约通过；PostgreSQL 测试需要隔离测试库 |
| Web 部署 | Docker 构建、隔离容器登录和资源读写、重启持久化 | Web Host 与静态前端协作通过；测试数据和临时容器已清理 |
| 桌面 Host | Tauri 格式、编译与桥接测试 | IPC 调用共享 API router；真实系统窗口、签名安装包与升级仍需平台验收 |
| 桌面更新 | Release 脚本、manifest 校验和更新状态单元测试 | 本地签名、跨平台构建和已安装版本升级需使用 GitHub 发布产物完成 |

## 持续验证

```sh
pnpm install --frozen-lockfile
pnpm release:check
pnpm test:release
pnpm --dir apps/desktop format:check
pnpm --dir apps/desktop check:dev-styles
pnpm build:checked
pnpm test:specx
```

PostgreSQL 与 TLS 集成测试仅在配置隔离的 `SPECX_TEST_POSTGRES_URL` 和测试证书
后才会实际执行；测试进程提前返回不表示生产数据库路径已验收。桌面安装、更新、
签名和重启需要在目标操作系统上验证，浏览器测试不能替代原生窗口验收。

## 当前边界

- 前端主包仍需随业务页面增长安排按需加载。
- 真实备份恢复工具链需在目标部署环境验收。
- Windows、Linux 与 macOS 安装包的完整升级链应使用上一正式版进行回归。
- 发布密钥、仓库 Secrets 和 Apple 公证配置按[版本与发布规范](guides/desktop-releases.md)维护。
