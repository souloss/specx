# Core 最小模型

以下模型是一期讨论和接口设计的最小集合，字段可随 Story 评审调整。

| 实体 | 关键字段 | 约束 |
|---|---|---|
| `workspace` | `id`, `tenant_id`, `name` | 属于一个租户 |
| `service_group` | `id`, `workspace_id`, `parent_id`, `name`, `position` | `parent_id` 不能形成环 |
| `service` | `id`, `workspace_id`, `group_id`, `name`, `slug`, `description`, `status` | ID 稳定；一期归属一个目录 |
| `tag` | `id`, `workspace_id`, `name`, `color` | 工作区内名称唯一 |
| `service_tag` | `service_id`, `tag_id` | 组合唯一，保证同一工作区 |
| `specification` | `id`, `workspace_id`, `service_id`, `kind`, `title`, `content_ref`, `content_hash` | 服务归属不可跨工作区 |
| `specification_revision` | `id`, `specification_id`, `content_ref`, `content_hash`, `created_by` | 内容变化时追加，历史版本不可覆盖 |
| `repository` | `id`, `workspace_id`, `provider`, `display_name`, `canonical_url` | 只登记用户已有的 HTTPS/SSH Git 仓库；工作区内规范 URL 唯一，凭据不落在本表 |
| `service_repository` | `id`, `service_id`, `repository_id` | 一个服务最多一条关联；不记录根路径或 primary/secondary 角色 |
| `specification_code_binding` | `id`, `specification_id`, `service_repository_id`, `path`, `ref_kind`, `ref_name` | 一份规范至多一条；path 非空表示仓库托管的 Markdown 文件，空 path 表示 SpecX 维护内容；同一服务下规范可选择不同 branch、tag、commit |
| `asset` / `source_spec`（后续） | `id`, `service_id`, `kind`, `name`, `path/glob` | 资产身份和仓库内具体文件范围属于后续分析能力 |
| `repository_snapshot`（后续） | `id`, `repository_id`, `resolved_revision`, `content_root_ref`, `manifest_hash` | 通用仓库控制器产出的不可变版本证据；下游控制器按快照读取文件 |
| `code_snapshot` / `analysis_result`（后续） | `id`, `repository_snapshot_id`, `manifest_hash`, `result_ref` | 分析和规范上传引用不可变仓库快照，不由具体规范控制器重新拉取仓库 |

服务名用于展示，`slug` 只用于稳定的人类可读引用，授权和外键使用 UUID。标签筛选是查询条件，不复制服务记录。`content_ref` 可以是内存正文或对象存储引用，但读取都经过 Core 授权。

`Service` 不保存仓库 URL、目录、语言或 CI 标识。语言和框架属于代码快照的可变分析
事实；CI 和规范来源通过 Service/SpecificationCodeBinding、Asset、快照 ID 和独立关系表扩展。仓库托管规范的内容、路径、版本和删除/恢复操作受来源同步约束。旧的
`code_source` / `service_code_binding` 表仅用于迁移兼容，不再作为新功能建模入口。仓库快照、
规范正文和分析结果的物理内容由平台内部 ContentStore 统一选型，业务模型只保存内容引用、
hash、大小和生命周期元数据；StorageClass/Claim 不作为普通用户资源。
