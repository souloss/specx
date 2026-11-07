# 当前架构决策

状态：specX 服务目录一期基线。

## 产品与技术基线

| 议题 | 当前决策 |
| --- | --- |
| 中心对象 | 多租户工作区中的服务，以及服务下的目录、标签和规范 |
| Core | Rust 模块负责工作区、服务目录、标签、规范、授权、审计和持久化 |
| Web Host | Axum 服务负责登录会话、请求上下文、静态文件和 API 挂载 |
| 前端 | `apps/desktop` 中的 Vue 3 + Pinia 共用应用；通用控件、主题、目录状态和资源视图分层维护；Web 与 Tauri 共用 |
| 规范 | 一期内置 Markdown 预览，未来格式通过受控贡献接口增加 |
| 存储 | SQLite 用于本地和单机部署，PostgreSQL 用于共享部署；可选备份存储不保存唯一事实 |
| 部署 | 仅提供 Dockerfile 和 Docker Compose；前端、API 和 Web Host 合并为一个服务 |

桌面采用独立的 `src-tauri/` 主机，而不是 `crates/` 中的业务 crate。它负责进程生命周期、平台路径和 IPC 转换，并将请求送入 `specx-api` 的同一 router；因此桌面与 Web 的授权、审计、幂等和并发语义一致。

前端按组件、状态、资源适配、纯算法和运行环境分层。通用模块不依赖资源领域，
资源适配集中在 store 与 `lib/sidebar`。模块职责和扩展规则见
[UI 基础层](ui-foundation.md)。

## 必须守住的边界

1. 服务不是连接、仓库、部署实例、环境或凭据。服务身份独立存在，外部来源通过明确关联接入。
2. Core 管理业务事实和授权；Web Host 管理会话与 HTTP；前端不直接访问持久化层。
3. 目录、服务、标签和规范列表是工作区共享事实；标签页、筛选和面板尺寸是用户偏好。
4. 所有接口和查询都按租户、工作区和主体授权；客户端传入的 workspace ID 不能单独构成授权依据。
5. 持久化数据库是 specX 自身的数据存储，不提供数据库连接、SQL 编辑、驱动或数据库管理能力。

## 尚未冻结的决策

- 规范编辑和修订历史的完整交互。
- 私有仓库授权、代码采集和外部 Provider 集成的具体协议。
- 大文档和附件的物理存储实现与垃圾回收策略；存储后端由平台统一选择，不作为普通资源的用户配置。

## 已冻结：Service 与 Repository 解耦

- `Service` 是规范与代码分析的治理主体；不直接保存 URL、路径、语言、凭据或 CI 信息。
- `Repository` 是工作区内按规范化 Git URL 去重的可复用登记项；SpecX 不创建远程仓库。
- 一个 Service 最多关联一个 Repository；Repository 可由多个 Service 复用。
- `ServiceRepository` 只表达仓库关联，不带 root path；branch、tag、commit 和规范托管时的 Markdown 相对路径属于具体规范的 `SpecificationCodeBinding`，同一服务下不同规范可选择不同 ref。
- 规范创建时明确选择 SpecX 维护或仓库同步。仓库同步规范的内容、路径、ref、恢复、删除和解除绑定由同步来源控制；语言/框架属于代码分析事实；凭据属于未来独立的 Integration/Secret 能力。
- 旧 `CodeSource` / `ServiceCodeBinding` 表和 API 仅用于兼容过渡，不再扩展；旧数据回填规则见[旧代码绑定说明](code-bindings.md)。
- 当前持久化、API 和桌面管理边界见[服务、仓库与代码资源管理](service-code-management.md)。

## 目标决策：控制器驱动的能力组合

- 平台统一定义内容存储策略。规范正文、附件、仓库快照、渲染缓存和分析产物通过内部 `ContentStore` 保存；业务资源只持有 `ContentRef`、hash 和恢复元数据。普通用户不创建 StorageClass、StorageClaim 或 StorageBinding。
- 外部能力通过 Controller 组合。Controller 观察资源和上游不可变快照，调用 Provider，按 generation/幂等键写回状态和审计；Provider 不直接写 Core，也不直接调用下游领域 Controller。
- 仓库同步先由通用 Repository Controller 负责仓库、ref、Webhook/轮询、resolved commit 和 `RepositorySnapshot`。Markdown Document Controller 只消费快照和 `SpecificationCodeBinding.path`，读取文件并追加规范修订。
- Webhook、轮询和队列事件只是协调触发器，资源、快照和状态才是事实来源；事件丢失或重复不能破坏最终一致性。
- Controller 先作为 Host 内的受控任务落地；在出现第二个真实消费者和明确隔离需求前，不引入通用插件市场、独立控制器集群或任意资源调度框架。

控制器边界、快照模型和失败恢复约定见[控制器与同步链路](controller-model.md)。
