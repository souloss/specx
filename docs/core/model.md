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

服务名用于展示，`slug` 只用于稳定的人类可读引用，授权和外键使用 UUID。标签筛选是查询条件，不复制服务记录。`content_ref` 可以是内存正文或对象存储引用，但读取都经过 Core 授权。
