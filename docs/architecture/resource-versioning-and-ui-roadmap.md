# 统一资源版本模型与页面边界路线图

状态：平台架构决策草案。用于约束服务、规范实例、Provider 投影和页面职责，并拆分后续可验收 Story。本文不表示路线图中的目标行为已经全部实现；实现状态以代码、API 契约和迁移为准。

## 目标

SpecX 以服务为治理入口，以统一规范资源管理为平台主线。OpenAPI、Markdown 以及后续 Provider 可以增加自己的内容解释和交互，但不能各自定义一套平台身份、授权、历史和导航规则。

近期工作的重点是补齐平台资源模型和页面契约，修复节点可导航、可查看、交互一致的问题。OpenAPI 已有能力作为 Provider 接入和边界验证对象；在平台抽象与页面框架稳定前，不继续扩充它的专属功能清单。

本文与[平台资源与能力设计总纲](platform-design.md)、[Core/Host/Controller/Provider 边界](provider-boundaries.md)、[规范与面板扩展模型](extension-model.md)、[控制器与同步链路](controller-model.md)配合使用。OpenAPI 特有的解析和视图约定见[OpenAPI 规范管理技术方案](openapi-specification-design.md)。

## 一、资源与节点模型

需要区分持久资源和导航投影。侧边栏里看起来是父子节点，不代表每个节点都必须成为数据库资源。

| 概念 | 身份与生命周期 | 内容与版本 | 页面责任 |
| --- | --- | --- | --- |
| Workspace | 平台持久资源；隔离和协作边界 | 不承载规范内容版本 | 工作区切换及工作区级能力 |
| Service | 平台持久资源；服务 ID 稳定 | 描述、标签、仓库关联等元数据可变 | 服务概览和服务级通用操作 |
| Provider Category | 由 Provider 注册信息和服务下实例投影出的虚拟目录 | 无独立内容、无版本树 | 分类概览、实例清单和该 Provider 声明的服务级设置入口 |
| Specification Instance | 平台持久资源；ID 表示一个规范实例 | 正文通过不可变 Revision 演进 | Provider 渲染实例主体；平台提供来源、历史、标签及通用操作框架 |
| Specification Revision | 由单个实例拥有的不可变子资源 | 保存正文快照、hash、父修订和来源证据 | 历史查看、比较或恢复的输入；历史修订只读 |
| Provider Projection Node | Provider 从特定规范修订生成的目录或叶子视图 | 不独立持久化，不拥有版本树 | 由 Provider 渲染目录、Operation、Schema 等专属视图 |
| Service Release | 可选的不可变服务级清单 | 固定来源 commit 与多个实例 Revision 的组合 | 整体发布、回滚、分享和审计的固定入口 |

逻辑关系如下。Provider Category 和 Projection Node 是树上的视图节点，不是额外的持久层级。

```text
Workspace
└── Service（稳定身份；平台元数据）
    ├── ServiceRepository / SpecificationCodeBinding（来源关联与同步策略）
    │   └── branch/tag 解析为具体 commit
    ├── Provider Category（虚拟目录）
    │   └── Specification Instance（稳定身份）
    │       ├── Specification Revision 1（不可变）
    │       ├── Specification Revision 2（不可变）
    │       └── Revision-scoped Provider projections
    └── Service Release（可选、不可变的组合清单）
        ├── Repository commit
        ├── Specification A revision
        └── Specification B revision
```

### 身份与可变性规则

- Service 与 Specification 的 ID 不因改名、移动、Provider 重解析或修订增加而改变。
- 服务和实例的描述、标签、展示元数据、来源配置是可更新的资源事实，应使用现有并发与审计规则。
- Specification Revision 一经创建不得原地修改。手工编辑、恢复、导入或同步产生新修订；恢复是追加一个内容等于历史版本的新修订，不是把当前指针倒回后覆盖历史。
- Repository branch 是可移动引用；tag 名称也必须解析并记录 commit，不能只把名称当不可变证据。Git commit 和已保存的仓库快照是不可变来源事实。
- 目录、Tag、Operation、Schema 等投影节点由 Provider 按指定 Revision 计算。它们可使用稳定的 `identityKey` 支持导航，但删除或改名遵循规范内容的修订，不执行独立的资源删除或历史恢复。
- Service Release 创建后不可变。它只引用规范修订和来源证据，不复制正文；不能包含 Secret 明文。若将来要求环境参与可重现发布，应引用经过版本化的非敏感配置并单独定义 Secret 解析策略。

## 二、三种“版本”不可混用

### 来源版本：branch/tag 与 commit

branch/tag 是用户选择或控制器跟踪的来源引用。系统解析后得到 `resolvedCommit`，RepositorySnapshot 以仓库、commit 和内容清单为依据去重。控制器和 Provider 消费不可变 Snapshot，不把 branch 名称作为内容版本。

服务与仓库的治理关系由 `ServiceRepository` 表达；规范托管的路径和 ref 由 `SpecificationCodeBinding` 表达。服务仓库策略可以提供默认 ref，但若具体规范需要不同 ref，应由显式的实例来源策略表达。当前实现若已允许实例独立选择 ref，迁移前必须盘点 API 与数据，不能静默将其改成服务统一 ref。

界面若提供 branch/tag 选择，只能称为“代码来源版本”或“仓库快照”，并明确其作用范围。它可以切换仓库托管规范的来源上下文；它不能声称同时切换手工维护规范。用户在 UI 中临时查看另一快照不应自动改写服务的跟踪策略或制造新的内容修订。

### 实例版本：Specification Revision

每个规范实例维护自己的修订序列。这样可单独查看、比较和恢复一份规范，不会因为同服务另一份规范发生变化就制造无关修订。仓库同步产生的修订应记录来源 RepositorySnapshot、commit、路径和内容 hash；内容不变时只更新同步观察证据，不追加重复正文修订。

实例版本号只在该实例内部有意义，例如 `A@r7` 和 `B@r7` 不代表同一次服务发布。历史修订和当前修订可被统一页面框架展示，但写入权限由来源策略和修订状态决定。

### 服务版本：Service Release

实例修订无法单独表达一组规范在同一时点的组合。需要发布、回滚或审计整个服务时，创建 `ServiceRelease`，其清单固定：

- 每个仓库的 `resolvedCommit` 及快照标识；
- 每个规范实例所选的 Specification Revision；
- 清单自身的创建者、时间、说明和内容摘要。

因此推荐的关系是：**实例修订是日常内容历史；服务发布是可选的组合快照；来源 ref 是快照的输入上下文。**不以“服务每次有任意改动就递增服务版本”替代实例历史，也不强迫普通编辑先创建发布。

第一阶段可不建设 ServiceRelease，但 API、UI 命名和数据库设计不得把 Git branch 或某个 Specification Revision 伪装成整个服务的版本。未来整体版本选择器应选择 Release；若选择仓库来源，则必须清楚标识为来源快照并限定影响范围。

没有 ServiceRelease 时，Service Overview 和 Provider Category 展示的是服务下各实例当前有效修订的聚合结果。它适合日常管理，不承诺跨实例的一致时间点；某个实例同步成功而另一个实例仍处于旧修订或失败状态，页面必须分别显示状态和来源证据。只有打开或发布 ServiceRelease，平台才可以把多个实例和仓库来源固定成一个可重现的组合。

## 三、Provider 与平台的页面契约

### 平台负责

- Service、Specification 等真实资源的身份、归属、授权、来源、通用历史与审计事实。
- 应用导航、面包屑、资源标题区、标签、加载/错误/空状态、并发冲突和通用确认流程。
- 根据资源类型与能力声明提供适用的编辑、同步、历史、解除来源等动作；动作是否可用不由页面临时猜测。
- 通用目录承载、节点展开、选择、打开、搜索、排序和标签页生命周期。
- Service Overview 展示服务元数据、仓库绑定状态和规范分类摘要，不把 Markdown/OpenAPI 等 Provider 专属字段写死在 Service 页面。

### Provider 负责

- 注册稳定的 `providerKind`、名称、图标和支持的 Specification kind；决定本服务是否应显示自己的虚拟分类目录。
- 将一个或多个规范实例投影为受限的 directory/leaf 节点，提供稳定 `identityKey`、父子关系、来源实例、Revision 上下文及可用能力。
- 渲染该 Provider 的分类概览、实例视图和专属投影节点视图；解释规范语义并生成对应内容。
- 声明专属操作，例如 OpenAPI 的文档预览、设计或调试入口。平台用能力声明提供统一入口和权限门控，不把这些操作塞进所有规范页面。

### 共享框架的页面规则

1. **任何可见节点都可打开。**目录节点的展开箭头只负责展开；点击名称打开该节点自己的概览/目录页面。叶子节点打开其详情视图。不能出现“侧栏有节点但点击没有页面”的状态。
2. **Service 页面是平台概览。**绑定仓库、创建规范可以分别由服务创建流程、顶栏主操作或右键菜单提供；Overview 不重复堆叠所有命令。页面应一眼看见仓库是否绑定、规范分类及数量、服务描述和标签。
3. **Provider Category 必须有概览。**显示 Provider 身份、规范数量、状态摘要、实例列表和该 Provider 的全局设置入口；没有该类型实例时可不创建目录，或由产品导航明确提供创建入口，但行为必须一致。
4. **Specification 页面由 Provider 呈现正文。**平台提供统一资源头部与通用动作，Provider 决定正文布局和专有 Tab。Markdown 实例可以是标题叶子；OpenAPI 实例可展开为 Tag/Operation 等投影。
5. **投影节点总是绑定 Revision。**路由和标签页上下文至少包含实例 ID、稳定节点键和 revision/content hash。切换规范版本后重新计算投影；当前 revision 已不存在该节点时显示“该版本中已移除”，不得静默跳到同名节点。
6. **平台操作由声明能力驱动。**节点声明 `open`、`edit`、`sync`、`history`、`execute` 等能力；授权和来源规则仍由服务端校验。Provider 不能通过 UI 能力声明绕开平台权限。
7. **布局规则统一。**主面板外层占满可用宽度并负责滚动，内层内容容器负责最大宽度与居中。Service、分类概览、规范和投影详情使用同一资源页面骨架，滚动条贴近主面板边缘。

页面和标签页使用统一的 `ResourceViewContext`，至少包含：

```text
workspaceId
serviceId
providerKind?          # 分类或 Provider 投影视图需要时提供
specificationId?       # 规范或其派生节点需要时提供
revisionId/contentHash? # 当前实际展示的不可变内容上下文
projectionKey?         # Provider 生成的稳定目录/叶子键
releaseId?             # 从服务发布清单打开时提供
```

`revisionId` 或 `contentHash` 缺失时，Provider 只能展示当前摘要，不能把一个旧标签页中的节点内容复用到新版本。`releaseId` 存在时，所有实例都必须按清单选择修订；清单中缺失的实例显示明确的发布不完整状态。路由可使用领域专用 URL，但不能只用标题、路径或当前数组下标作为投影身份。

实现先采用静态 Provider 注册表和类型化接口，不引入运行时任意插件或通用 CRD 执行环境。基础控件、平台资源框架和 Provider 专属视图分层；Provider 不直接访问 Core 数据库，也不拥有授权、修订或工作区事实。

## 四、实施路线

| 阶段 | 交付内容 | 验收标准 |
| --- | --- | --- |
| 0. 盘点与冻结契约 | 对照现有 Service、Specification、Revision、RepositorySnapshot、导航节点和标签页；确定身份、版本上下文、节点能力及来源约束；记录兼容迁移方案 | 每类对象明确 owner、是否持久化、版本语义、读写者；现有 API 行为与目标差异有清单；本阶段不扩展 OpenAPI 专属功能 |
| 1. 导航与视图闭环 | 统一节点描述与打开路由；目录支持展开和打开；Service、Provider Category、Specification、Projection Node 都有页面责任；补齐 Markdown 分类概览 | 资源树任意可见节点都可打开；刷新/恢复标签页仍指向相同资源与节点；过期投影显示版本失效提示；空态和错误态可恢复 |
| 2. 平台资源页面框架 | 抽取资源头部、面包屑、来源/状态摘要、通用动作、加载/错误/空状态和滚动容器；Service Overview 去重并动态展示 Provider 分类 | Markdown 与 OpenAPI 使用相同平台框架；按钮、菜单、表单和标签样式一致；窄窗口、键盘导航、中英文和主题均可用；滚动条贴近主面板右边缘 |
| 3. Provider 视图接入 | 固化静态 Provider 注册、分类元数据、导航投影及 `ResourceViewContext` 契约；让 Markdown 与 OpenAPI 分别提供分类、规范和专属节点视图 | Core/UI 平台层不判断 OpenAPI Tag/Operation；Markdown 分类概览可用；OpenAPI 分类、规范、Tag/Operation 视图都从选定 Revision 投影；Provider 故障时仍可读资源元数据与原始规范 |
| 4. 版本上下文闭环 | 明确当前 Revision、历史 Revision、仓库来源 Snapshot 的选择与只读行为；修订/来源状态进入通用页面框架；来源恢复写入新修订 | 实例历史可独立查看；仓库托管版本显示 ref 与 resolved commit；切换来源快照不改变实例跟踪配置；SpecX 托管实例不受 branch 切换影响；内容 hash 不变不重复造修订 |
| 5. 服务级发布快照（按需） | 仅在整体发布、回滚或审计成为明确需求后增加 ServiceRelease 清单和服务级发布视图 | 清单固定所有来源 commit 与实例修订；发布历史不可变；可由清单重建 Provider 投影；发布切换不会改写各实例历史；Secret 不进入清单 |

阶段 0 至 4 是统一资源管理与 Provider 边界闭环的主线。阶段 5 是可扩展的下一层，不是首轮平台重构的前置条件。每阶段先写可观察 Story 和接口契约，再做实现；除非盘点发现不可兼容问题，不进行无收益的通用资源表或数据库重构。

## 五、近期 Story 顺序

1. **资源/节点目录与版本字段盘点**：标出现有持久对象、虚拟节点、内容存储引用、修订来源和前端导航身份；确认当前每个 Specification 是否能独立选择 ref。产出兼容矩阵，不改代码行为。
2. **节点路由与打开语义**：把“展开”和“打开”分开；为 Service、Provider Category、Specification 和 Projection Node 定义稳定路由上下文；实现所有目录节点的概览页面。
3. **通用页面骨架与 Service Overview 收敛**：平台负责资源标题/来源/标签/通用动作；Overview 只显示有价值的服务信息和 Provider 分类摘要，仓库绑定/创建等命令放到一致的主操作或上下文菜单。
4. **Provider Category 概览**：以注册元数据生成图标、目录名和分类页；聚合该 Provider 下规范实例及状态；提供 Provider 声明的服务级设置入口。
5. **Markdown Provider 迁移验证**：多个 Markdown 实例进入同一分类目录；分类可查看；实例仍能阅读、编辑、查看历史或按来源约束只读。
6. **OpenAPI Provider 边界验证**：停止增加与平台目标无关的 OpenAPI 专属能力；将规范、Tag、Operation 页面接入统一资源框架；Operation 视图仅消费当前规范 Revision 的局部投影。
7. **历史与来源统一交互**：通用历史面板显示实例修订与仓库来源证据；只读、恢复、同步和解除来源根据资源能力及服务端授权展示。
8. **服务发布快照需求评审**：用真实发布/回滚场景验证是否需要 ServiceRelease；若需求仅为切换仓库 branch，不把它提升为服务整体版本模型。

## 六、跨层验收

- **模型与 API**：Service、Specification、Revision、RepositorySnapshot 和投影节点身份不混淆；API DTO、ETag/并发、授权和错误契约一致；SQLite/PostgreSQL 行为一致。
- **来源与历史**：branch/tag 只作为 ref，保存解析 commit；内容 hash 去重；历史恢复追加新修订；历史内容只读；手工实例不因仓库更新而变化。
- **导航**：服务可展开；分类可打开；多个同类规范稳定排序；目录点击打开概览、展开按钮只展开；多 Tag 别名仍打开同一 Operation 身份；过期节点不会误跳到同名节点。
- **交互与视觉**：Service、分类、规范和叶子视图的标题、工具栏、按钮、表单、菜单、空态和滚动规则一致；验证桌面和窄窗口、中英文、亮暗主题、键盘与鼠标操作。
- **Provider 隔离**：平台层不解析具体规范格式；Provider 不写 Core、不自管权限、不创建投影节点版本表；静态注册缺失或 Provider 错误时，导航和原始资源仍有清晰的回退状态。
- **回归**：Markdown 创建/编辑/同步/历史与来源约束不退化；OpenAPI 原始定义、当前 Operation 文档视图与既有仓库来源按当前契约工作；运行受影响的前端、Core/API、迁移及契约检查。

## 七、当前不做

- 不为每个 Provider Category、Tag、Operation 或 Schema 建立独立版本树。
- 不将每次普通编辑强制升级成服务版本，也不把 Git branch/tag 当成整个服务的发布版本。
- 不为第一版统一资源模型引入万能 EAV/JSON 资源表、动态 CRD、任意插件市场或独立 Provider 进程。
- 不把 OpenAPI 调试、生成代码、自动化测试、压力测试等 API 生态能力作为当前平台抽象的验收前提；它们应由后续用户场景逐项立项，并通过 Provider/平台已有边界接入。
- 不将 ContentStore 后端选择暴露为普通用户的 StorageClass/Claim 操作；存储仍由平台统一配置和选择。
