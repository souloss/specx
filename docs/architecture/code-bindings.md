# 旧代码来源与绑定接口

状态：兼容迁移说明。新功能使用[服务、仓库与代码资源模型](service-code-management.md)中的 Repository、ServiceRepository 和 SpecificationCodeBinding；不要再扩展旧 `CodeSource` / `ServiceCodeBinding` 表单或 API。

旧接口为早期原型开放了 `POST /code-sources`、`PUT /code-sources/{sourceId}`、`POST /services/{serviceId}/code-bindings` 等操作。它要求用户手动填写 Provider、显示名称、唯一标识、执行范围、实体类型和路径，职责与用户心智负担过重。新界面不再调用这些接口。

数据库迁移保留旧表，以避免升级时物理删除历史记录。迁移时：

- 仅把旧 Git 来源转换为 Repository；本地文件来源继续留在旧表中，不能伪装成远程 Git URL。
- 每个旧服务最多选一个迁移关联，优先选 primary，再按创建时间和 ID 稳定排序。
- 旧 branch、tag、commit 和 default-branch selector 仅保留在兼容迁移表中；新的规范绑定由用户在具体 Specification 上重新选择。
- 旧的额外来源和目录范围仍留在兼容表中，暂不参加新 ServiceRepository 查询。

旧 API 暂时保留，供已发布客户端过渡；API 删除与旧数据清理属于单独的兼容性变更。新业务不得依赖旧接口，旧服务/绑定也不再由新 UI 编辑。
