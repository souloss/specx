# 服务中心领域模型

## 稳定对象

| 对象 | 所有者 | 一期含义 |
|---|---|---|
| Tenant | Core | 多租户隔离边界 |
| Workspace | Core | 成员协作和服务资产边界 |
| Service | Core | 用户组织和进入能力的中心对象 |
| ServiceGroup | Core | 可无限嵌套的目录节点；一个服务一期归属一个目录 |
| Tag | Core | 工作区内可复用的标签；一个服务可有多个标签 |
| Specification | Core + 规范贡献 | 服务下的规范身份、标题、类型和内容引用 |
| SpecRevision（可选） | Core | 内容变更的不可变快照；是否进入一期由 Story 评审决定 |
| PanelSession | Host | 用户打开的服务面板标签页和恢复状态，不是业务事实 |
| Repository | Core | 工作区内按规范化 Git URL 去重的已有仓库登记项 |
| ServiceRepository | Core | 服务与唯一 Repository 的关联；不携带根路径或 primary/secondary 角色 |
| SpecificationCodeBinding | Core | 规范在所属服务仓库上的 Markdown 路径与 branch、tag 或 commit 选择器 |
| Asset / SourceSpec（后续） | Core/Provider | 服务下的资产身份及其文件选择规则 |
| RepositorySnapshot（后续） | Core/Controller/Provider | 通用仓库控制器解析出的不可变 commit、内容引用和来源证据 |
| CodeSnapshot（后续） | Core/Provider | 基于 RepositorySnapshot 的代码分析版本和结果证据 |

服务的稳定 ID 不随名称、目录移动、标签修改或来源同步改变。规范属于服务，但规范内容类型由 `spec_kind` 标识；Core 不解析 Markdown 语义，只保存经校验的内容和元数据。

## 服务与外部世界

服务身份和以下对象分开建模：

- 代码仓库：服务可以暂时没有仓库；一期一个服务最多关联一个已有 Repository，多个服务可以复用同一仓库。
- 部署实例：可以有多个环境和实例。
- 连接配置：需要凭据时由专门的受控能力管理，不写进服务描述。
- 外部采集来源：未来由 Asset/SourceSpec 和不可变代码快照记录，不用名称匹配覆盖人工服务。

服务和仓库通过 `ServiceRepository` 关联，而不是把 URL、路径或分支字段直接写入服务。
规范通过 `SpecificationCodeBinding` 选择开发分支、发布 Tag、固定 Commit 和（仓库托管模式下）单个 Markdown 文件；同一
服务下不同规范可以选择不同 ref。同一
仓库中的目录或 glob 文件范围属于后续 Asset 的 `SourceSpec`。语言、框架等会随代码版本变化的
事实属于后续代码快照分析结果。

未来 GitHub/GitLab 采集应由通用 Repository Controller 产生仓库快照或导入候选，先做去重和打标预览，再由用户确认写入服务目录。Webhook 和轮询只触发控制器，不能直接改变规范内容。

## 资源引用

跨模块使用 `ResourceRef`：`tenant_id`、`workspace_id`、`resource_kind`、`resource_id`，需要时增加不可变内容快照引用。引用方不能只依赖名称、目录路径或标签判断身份。规范预览固定服务、规范 ID 和内容版本，切换服务时销毁或失效旧上下文。

## 命名与关联约束

逻辑类型使用 `specx.service.*`、`specx.specification.*` 等语义命名；命名空间表达所有权，不要求每个命名空间拆成服务或数据库。跨域关系必须记录来源、目标和证据；一期不推断服务拓扑、字段血缘或业务概念关系。
