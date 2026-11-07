# Core、Host、Controller 与 Provider 边界

## Core

Core 是服务资产的事实源，负责租户和工作区上下文、服务目录、目录层级、标签、规范身份与内容引用、授权、审计和持久化。Core 提供稳定的查询和命令接口，不依赖具体前端框架或规范渲染库。

## Host

Host 负责应用壳：侧边栏、搜索筛选、顶部两侧工具栏、最小宽度约束、标签页、面板生命周期、主题和用户会话状态。Host 通过 Core API 读取服务事实，通过贡献接口挂载规范预览和未来专属面板。Host 不直接访问数据库或 Provider 私有 store。

## Controller 与 Provider/贡献实现

Controller 位于 Core 与 Provider 之间。它观察资源的期望状态和上游快照，选择能力实现，执行可重试的 reconcile，并通过 Core 命令写回 status、审计和不可变快照。Controller 可以先作为 Web Host 内的受控任务运行，不能直接连接数据库或绕过授权。

Provider 负责声明的格式或能力实现，例如 Markdown 预览、Git ref 解析、仓库快照读取和 Webhook 注册。它接收 Controller 发出的最小授权上下文，返回版本化结果、证据和诊断；不复制身份系统，不直接写核心表，不将服务凭据暴露给浏览器。Provider 不直接调用另一个领域 Controller。

一期不要求独立 Controller 集群、Provider 进程、复杂插件包或插件市场。纯 UI 的 Markdown 贡献可以随 Web 发布；仓库同步先由内置 Controller 调用受控 Git Provider 完成。未来涉及不可信代码、用户输入执行或高资源消耗时，再使用受管进程或执行池隔离。

代码来源 Provider 遵循相同边界：Core 只保存已有 Git `Repository`、服务关联
`ServiceRepository` 和规范级 `SpecificationCodeBinding`。SpecX 不创建远程仓库；仓库 fetch、
ref 解析、快照生成和受控文件读取由 Repository Controller 调用 Git Provider 完成。
Controller 产出的 `RepositorySnapshot` 是下游控制器的交接事实。Markdown Document Controller
再根据 `SpecificationCodeBinding.path` 读取快照并追加规范修订。规范托管的单个 Markdown
路径由 binding 记录；目录和 glob 等具体文件范围留给后续 Asset 的 `SourceSpec`，凭据留给
独立的 Integration/Secret 能力。

## 请求方向

```text
Web Host ── Core API ── Core domain/storage
    │                     │
    │                     └── Controller ── Provider ── Git / storage / analysis
    └──── contribution bridge ── Markdown renderer / future provider
```

核心事实流向是 Core → Host/Controller/贡献；Controller 和贡献不能反向改变服务身份、目录权限或工作区边界。Webhook、轮询和任务事件只负责唤醒 Controller，不能代替资源、快照和状态事实。Provider 或 Controller 的不可用状态要能被用户理解，并且不会阻塞服务目录和已有规范读取。
