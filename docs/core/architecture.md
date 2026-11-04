# Core 架构

Core 是服务资产工作台的事实层。它不负责绘制侧边栏或决定某个规范的视觉样式，也不把某个 Provider 的领域模型编译进核心。

## 模块

| 模块 | 事实或能力 |
|---|---|
| Tenant/Workspace | 租户、工作区、成员上下文和授权边界 |
| Catalog | 服务、目录树、目录排序和归档状态 |
| Tagging | 工作区标签和服务的多对多关联 |
| Specification | 服务下的规范身份、类型、内容摘要和存储引用 |
| Authorization | 资源读取/写入、工作区角色和审计 |
| Contribution registry | 规范预览和未来面板的贡献声明与可用状态 |

Rust 代码按边界落在 `specx-types`、`specx-formats`、`specx-platform`、`specx-core`、`specx-api` 和 `specx-web` 六个 crate 中，原生桌面壳独立放在 `src-tauri/`。共享领域数据契约在 `specx-types`，Core 拥有规则与持久化，纯内容处理在 `specx-formats`，进程路径和数据库 URL 语法在 `specx-platform`。API 资源处理器、请求/响应模型和 OpenAPI 描述目前位于 `specx-api/src/resources.rs`，后续按资源增长再分文件；Web Host 的认证位于 `auth.rs`。前端 API client、类型、纯展示工具和界面组件分开，业务目录经 API 读写，只有面板会话等 UI 状态放在 `localStorage`。

API 从服务端会话构造主体，再校验所选工作区成员关系，随后调用 Core。浏览器可请求选择工作区，但不能自行指定租户或 actor 身份。插件和 AI 尚未接线；将来通过版本化贡献协议接入，不直接写 Core 表。

## 读取与写入路径

```text
请求 → 租户/工作区上下文 → 授权 → 应用用例 → 仓储/对象存储
                                      └→ 贡献调用（可选）→ 结果诊断
```

创建服务、移动目录、修改标签和保存规范元数据是同步 Core 用例。Markdown 渲染可以同步完成，也可以异步缓存；渲染失败不回滚已经保存的服务事实。

## 生命周期

服务可以创建、修改、归档和恢复；归档后保留可读历史，阻止新内容写入。规范是否支持编辑和修订要由一期 Story 决定；无论采用何种方式，预览都必须绑定明确的内容版本，不能读取用户正在编辑的另一份内容。
