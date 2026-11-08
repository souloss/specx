# OpenAPI 规范管理技术方案

状态：实施中。OpenAPI 类型、创建/更新 API、仓库文件发现、可展开的服务规范树、JSON/YAML Tag 导航、单操作参数/请求体/响应预览，以及支持 JSON/YAML 高亮与格式化的编辑器已接入。服务环境和 Try it 仍按后续 Story 实施。

## 目标

在现有服务、Markdown 规范、仓库快照和修订能力上，增加 OpenAPI 规范的完整生命周期：创建或导入、解析校验、查看、编辑、仓库同步、历史恢复、按 Tag 浏览 API 定义，以及基于服务环境发送请求。

创建流程默认从本地文件导入，并复制为由 SpecX 管理的规范内容；用户可自定义导入后的规范标题。也可以选择服务仓库中的 branch/tag 和文件路径作为持续同步来源，或切换为直接在 SpecX 中录入。各来源按规范类型校验文件扩展名与内容，仓库路径和版本在保存前校验。

开始 OpenAPI 纵向开发前，先完成资源导航与内容加载的基础调整。OpenAPI 不能被当作另一种 Markdown 预览：它需要按规范结构生成目录和操作视图，同时保留原始定义作为可编辑、可同步、可恢复的权威内容。

## 当前实现检查

| 边界 | 当前事实 | 对 OpenAPI 的影响 |
| --- | --- | --- |
| 服务目录 | `ServiceGroup` 支持无限层级；服务可展开，规范实现将分类、规范、Tag 和操作节点投影到侧栏。 | Core 继续只保存真实服务和规范；虚拟节点由规范实现生成并通过稳定身份打开对应视图。 |
| 规范模型 | Core 的 `SpecificationKind` 支持 Markdown 与 OpenAPI；正文和修订作为文本保存。导航标签可指向服务、规范、分类/Tag 目录或具体操作。 | 按规范类型解析和呈现；OpenAPI 操作视图仅展示选中操作，不重复展示整个文档。 |
| 规范 API | 规范创建/更新、ETag、修订、恢复、仓库来源及 OpenAPI 语义校验已接入。 | 复用生命周期和并发语义；Markdown 保持原行为，OpenAPI JSON/YAML 在写入边界校验。 |
| 仓库同步 | Repository Controller 提供通用文件快照；文件发现接口列出 Markdown、JSON 和 YAML 文件，来源路径与正文按规范类型校验。 | 文档同步按规范类型消费通用快照；不让 Repository Controller 理解 OpenAPI 语义。 |
| 目录载荷 | `/catalog` 中每个 `SpecificationResponse` 都带完整 `content`。 | 文档数量和体积上升后，启动和切换工作区会下载并保留全部正文；需要摘要列表和按需读取。 |
| 环境与执行 | 已有工作区级 Secret/Integration，凭据加密保存且列表只返回元数据；没有服务环境资源或 API 请求执行能力。 | 环境配置、Secret 使用范围和请求执行位置需要单独定义，不能复用 Git 凭据语义。 |
| 内容布局 | `ServicePanel` 的概览和规范面板同时承担 `max-width: 1100px` 和滚动容器职责；外层 main pane 还有响应式 padding。 | 窄内容容器居中时滚动条随容器居中，视觉上离主窗格右边缘较远。 |

`/api/specx/openapi.json` 是 SpecX 自身 HTTP API 的契约，不是用户服务的 OpenAPI 规范。

## 推荐资源树

把持久化资源关系和导航视图节点分开：Core 继续保存 Service、Specification 等有独立生命周期的资源；每个规范实现根据自己的内容返回服务下的导航投影，侧栏只负责承载、排序、筛选和打开这些节点。第一版不建立万能目录表，也不把每个 API 操作拆成数据库资源。

```text
Workspace
└── ServiceGroup（现有，可无限嵌套）
    └── Service（服务概览视图，可包含子节点）
        ├── Markdown Provider（自动创建，带提供者图标）
        │   ├── README（规范叶子）
        │   └── Guide（规范叶子）
        └── OpenAPI Provider（自动创建，带提供者图标）
            ├── OpenAPI Specification（规范目录/概览）
            │   ├── Tag Directory（目录视图）
            │   │   ├── GET /users（API 定义视图，可执行）
            │   │   └── POST /users（API 定义视图，可执行）
            │   └── 未分类 API（没有 Tag 时的保留目录）
            └── Other OpenAPI Specification（同一提供者目录下的另一份规范）
```

### 节点与身份

- `ServiceGroup` 仍然表示工作区中的服务组织路径；`Service` 不增加 `parentServiceId`。服务变成树中的可展开父节点，是因为它拥有规范子资源，不表示服务可以嵌套创建另一个服务。
- `Specification` 是服务下的真实子资源，但它如何出现在服务下面由规范实现决定。实现可以返回一个叶子、一个目录，或带有任意深度受限的目录/叶子组合；通用导航层不判断 Markdown、OpenAPI 或其他格式的语义。
- 每个规范提供者都必须在服务下提供一个稳定的虚拟提供者目录；目录名称和图标来自注册元数据，目录只在该服务拥有对应规范时创建。它保证同一类型的多个规范不会与其他提供者混在一起，也不把提供者事实写入 `Service`。
- Markdown 提供者目录下放置规范标题叶子；OpenAPI 提供者目录下放置规范目录、Tag 目录和操作叶子。其他提供者沿用相同投影契约，可以返回自己的目录和叶子结构。
- Tag 目录与 API 操作是由特定规范版本派生的虚拟导航节点，不单独写入 Core。工作区服务标签与 OpenAPI 的 `tags` 是两个不同概念，界面应使用不同样式或分组避免混淆。
- API 操作的稳定键使用规范 ID 加 HTTP method 和规范化 path。`operationId` 可用于显示、搜索和代码生成，但它在 OAS 中可缺省且不保证唯一，不能作为唯一身份。
- 一个操作带多个 Tag 时，各 Tag 下可以出现指向同一操作身份的导航别名；操作标签页去重。没有 Tag 的操作放入默认目录。Tag 顺序优先使用文档声明顺序，未声明的 Tag 再按稳定规则排序。
- 所有目录节点都可以展开，也可以打开自己的目录/概览视图；叶子节点打开自己的详情视图。展开控件和选中行为必须分离，避免目录节点只能展开而无法查看。

规范实现应提供一个通用的导航贡献契约，概念上包含：`providerKind`、可选 `categoryKey`、节点类型（`directory`/`leaf`）、显示信息、稳定 `identityKey`、父节点、来源 `specificationId`、内容 hash 和节点能力（打开、编辑、同步、执行等）。`ResourceTreeNode` 和面板标签应消费这个判别联合，而不是写死 `openapi-tag`、`openapi-operation` 等业务类型。派生节点引用其来源规范 ID 和内容 hash，不依赖标题、分类名或当前数组位置作为身份。

工作区切换和内容更新时，旧派生节点需按 hash 失效；若同步删除了已打开节点，标签显示“定义已从当前版本移除”，不能静默切换到同名节点。目录节点的视图由规范实现提供，通用层只传递节点引用和能力声明。

此方案将“服务虚拟路径”解释为服务成为规范实现投影的承载父节点，并复用现有分组构成服务之前的目录路径。服务不承担规范语义，也不支持在服务下创建嵌套服务；不同规范类型、文件分类和内容结构由对应实现返回。

## OpenAPI 文档与生命周期

### 权威内容

`Specification` 继续作为规范身份、类型、服务归属、当前内容摘要、ETag 和修订历史的权威资源。增加 `OpenApi` 类型，正文保存用户原始 JSON/YAML；解析出的目录、Tag 和 operation index 是可重建投影，不替代原始正文。

写入和同步必须经过同一格式校验路径：

1. 校验大小、编码、JSON/YAML 语法和 OpenAPI 结构。
2. 生成可展示的规范摘要及操作索引；操作的 path、method、Tag 和 `operationId` 不满足基本约束时返回带字段路径的验证错误。
3. 只有验证成功才通过现有 ETag/CAS 事务更新规范与追加修订。
4. 对仓库托管内容，解析失败时保留当前有效版本和修订；将失败写入同步诊断，不把无效正文覆盖到当前资源。
5. 内容 hash 未变时更新同步证据但不追加重复修订。

首版建议支持 OpenAPI 3.0.x、3.1.x 的 JSON 和 YAML，拒绝或明确报告不支持的版本。`$ref` 首版只解析文档内引用，不从网络抓取外部引用；这样目录生成和验证不会触发隐式网络请求或 SSRF。解析需限制输入字节、引用深度和总操作数，并检测本地引用循环。

手动维护和仓库托管继续沿用现有来源模式。手动规范可编辑、查看修订并恢复；仓库托管规范只读、可同步和解除来源。更新型绑定使用 branch 或 tag；固定 commit 可作为历史/来源证据展示，不作为自动跟随配置。OpenAPI 文件候选扩展为 JSON/YAML 格式并通过解析确认，而不是仅凭扩展名判定有效。

### 视图

| 节点 | 主视图 |
| --- | --- |
| Service | 服务概览、仓库状态、规范入口、服务环境摘要 |
| Specification Provider | 提供者图标、名称、规范数量和状态摘要；由注册元数据稳定生成 |
| Markdown Specification | Markdown 阅读/编辑、修订历史和来源状态 |
| OpenAPI Specification | 基本信息、版本/服务器、Tag 与操作目录、来源/校验状态、原文查看/编辑 |
| Tag Directory | 该 Tag 下操作清单、method/path/summary 搜索和数量 |
| API Operation | method/path、描述、参数、请求体、认证要求、响应、示例及 Try it |

API 操作文档应包含 path/query/header/cookie 参数、请求体 media type/schema/example、响应 status/media type/schema/example、安全要求和服务端定义。首版不执行 OpenAPI `links`、回调、webhook，也不加载远程示例或 `$ref`。

## API 与存储基础调整

1. **摘要和正文分离**：为目录/树提供不含正文的规范摘要（ID、serviceId、title、kind、version、contentHash、updatedAt、来源摘要）；增加按 ID 读取正文的接口。为兼容现有 v1 客户端，可先增加显式 metadata 查询/目录接口并保留当前 `/catalog` 行为，后续再评估默认响应迁移。
2. **格式可判别**：规范创建请求显式包含 `kind`，旧请求缺省保持 Markdown；OpenAPI 记录 JSON/YAML 格式信息，更新时不允许悄悄改变 kind。DTO、ETag、错误目录、OpenAPI fixture 和双语验证错误同步更新。
3. **通用验证边界**：Core 负责不变量和事务；格式解析放在纯格式/文档解析能力中；API 在写入前调用；Controller 复用同一验证器。Core 的 `create_markdown_specification` 等专用方法演进为保持调用边界清晰的格式感知命令。
4. **仓库文件发现**：Repository Controller 仍只产出通用快照；文档发现按允许的文档类型筛选，Markdown 和 OpenAPI 分别校验路径、编码、大小和语义。新增 OpenAPI Document Controller 消费 RepositorySnapshot，记录 hash、commit、manifest 和诊断。
5. **实现导航投影与缓存边界**：规范实现返回的导航投影可按 `(specificationId, contentHash, providerVersion)` 在进程内缓存并随规范更新失效；持久索引只有在实测解析成本或搜索需求证明必要时再增加。禁止把缓存当作来源事实。缓存失效后必须能够从规范正文重新生成相同的节点身份。

现有 `specifications.kind` 已有文本列，Markdown 修订也保留原始文本；若不引入新持久关系，OpenAPI 类型本身不需要新表。摘要接口、环境资源和 Secret 关系仍需按 API/数据库变更规范评估 SQLite/PostgreSQL 迁移及 ETag 并发行为。

## 服务环境与请求执行

### 环境资源

新增服务归属的 `ServiceEnvironment`，建议字段包括稳定 ID、serviceId、名称、`baseUrl`、非敏感变量、认证模式、Secret 引用、是否生产环境、排序和更新时间。环境属于服务的共享配置，不放进 Specification，也不作为本机 UI 偏好。

密码、Bearer token、API key 等敏感值只能通过已有 Secret/Integration 机制引用；环境列表只返回 Secret 元数据/引用，不返回明文。创建或编辑环境使用 ETag/CAS；删除被正在使用的环境时要有明确行为。首版变量插值只允许显式的环境变量语法，不执行脚本、模板函数或任意表达式。

### 执行边界

请求执行是用户显式触发的一次性动作，不是 Core 事务，也不由 Repository/Markdown 控制器执行。首版不新增可接收任意 URL 的 SpecX 服务端 HTTP 代理，以免把环境地址变成 SSRF 入口并让 Core/Web Host 承担外部网络副作用。

推荐通过宿主提供的 `ApiRequestExecutor` 能力执行：浏览器 Host 使用受浏览器 CORS/Private Network Access 约束的 fetch；Tauri Host 可提供本地请求适配器，但要有明确的 Tauri capability、超时和响应大小限制。发送前显示环境、解析后的最终 URL、HTTP method；修改性请求必须由用户按下发送操作触发，生产环境给予显著提示。记录状态码、耗时、响应头白名单和受限大小的响应正文；授权头、Cookie、Secret、变量值和请求正文不得进入日志、错误消息或遥测。

浏览器 CORS 会限制部分用户服务，Tauri 与 Web 的网络行为也不完全相同。若要求 Web 部署也能绕过 CORS 或访问内网目标，应单独设计受管理员控制的执行代理：目标 allowlist、DNS/IP 重绑定防护、禁止云 metadata/loopback 的默认策略、出站网络隔离、Secret 最小权限和完整审计；该代理不纳入当前推荐的首版。

## 滚动条布局修正

问题定位在 `ServicePanel.vue` 与 `resources.css`：概览/规范节点的滚动元素同时设置了 `max-width: 1100px` 和水平居中，滚动条因此固定在居中内容框边缘；main pane 额外 padding 又缩小了滚动视口。

建议拆成两层：外层视口占满 main pane 可用宽度并负责纵向滚动；内层内容容器负责 `max-width`、居中和左右 padding。这样内容仍保持易读宽度，滚动条贴近应用主面板右缘。Service Overview、Markdown、OpenAPI 文档和 API 操作视图共用同一滚动/内容容器规则，避免每种面板各自修补。

## 分阶段实现与验收

### Story 0：资源视图与规范实现导航基础

- 资源树可以表达并展开 Group → Service → 规范实现返回的分类/规范/派生节点；服务概览和所有目录节点可打开自己的视图。
- 每个已出现的规范提供者都返回一个自动创建的提供者目录；Markdown 规范是目录下叶子，OpenAPI 规范是目录下的规范/Tag/操作投影；通用树只消费节点能力和图标元数据。
- 同一提供者目录下的多个规范稳定排序且互不覆盖；提供者目录没有规范时不创建。
- 规范目录只加载摘要，打开后按需加载正文；旧 API 客户端行为保持兼容。
- 工作区切换、标签页恢复、搜索、键盘导航、右键菜单、未保存保护和窄屏布局仍正确。
- 修正滚动视口，桌面/窄窗口验证概览、规范页滚动条贴近主 pane 右侧且内容最大宽度保持一致。

### Story 1：OpenAPI 文档生命周期

- 创建、导入、编辑、ETag 冲突、修订查看/恢复和仓库来源按类型工作。
- JSON/YAML 与支持版本校验准确；无效创建/编辑不改变服务端内容；错误含字段路径但不回显 Secret。
- OpenAPI 文件可从 RepositorySnapshot 发现和同步；有效变更产生单一修订，无效或未变化输入不产生错误修订。
- Markdown 的创建、编辑、来源限制、历史和现有 API 契约回归通过。

### Story 2：规范实现导航与 Tag/API 浏览

- 不同规范实现返回的节点可以共存；分类目录、规范目录和叶子节点的身份、打开行为和能力声明稳定。
- 同名 Tag 聚合为一个目录；无 Tag、多 Tag、未声明 Tag 和操作重命名/删除均映射稳定。
- 点击 API 操作显示完整文档；method/path/search/tag 状态与规范 hash 一致；旧版本操作标签不会误指向新操作。
- 列表支持大型规范虚拟滚动或分页，不因数百/数千操作阻塞主线程。

### Story 3：服务环境和 Try it

- 服务可创建、修改、排序和删除多个环境；环境变量与 base URL 展开结果可预览。
- 发送前确认目标环境和 URL；参数、请求体、认证和媒体类型生成正确；响应状态、时长、取消/超时、大小上限和错误显示明确。
- Secret 只在执行授权时使用，不出现在列表、日志、历史记录或错误反馈；Web CORS/Tauri 网络差异有真实运行验收。
- 对 POST/PUT/PATCH/DELETE 不自动重试，不在打开页面时自动执行。

### 跨层检查

- Rust Core/API/SQLite/PostgreSQL、migration check、ETag/授权、错误契约和 OpenAPI fixture 同步。
- Controller 测试覆盖快照交接、有效/无效文件、重复事件、hash 去重和旧 generation 不覆盖。
- UI 覆盖中文/英文、亮/暗主题、键盘导航、菜单、编辑冲突、窄窗口和真实 HTTP 请求展示。
- 安全验收覆盖本地 `$ref` 循环、外部 `$ref` 拒绝、不安全 URL/跳转策略、响应超限、Secret 脱敏和恶意 OpenAPI 字符串不作为 HTML 执行。

## 首版不做

- OAS 2.0/Swagger 转换、远程 `$ref` 获取、规范合并/生成器、mock server、自动化 collection、浏览器 cookie 注入、Webhook/回调执行。
- 将每个 operation、Tag 或解析出的 schema 独立写入数据库；把 arbitrary Service children 做成可执行脚本；通过通用资源表取代现有领域表。
- 面向互联网开放的服务端请求代理和任意插件运行时。

## 需要对齐的决策

1. 是否确认服务只负责承载规范实现返回的子树，不在服务下继续创建嵌套服务？
2. 是否确认每个规范提供者都必须提供自动创建的目录、名称和图标，目录下再由实现决定规范/Tag/操作层级？
3. OpenAPI Tag 目录由规范内容自动生成是否足够，还是还需要用户手工维护的额外目录？
4. 首版是否接受 OAS 3.0/3.1、JSON/YAML、仅本地 `$ref` 的支持范围？
5. 首版 Try it 是否接受 Web 使用浏览器 CORS、Tauri 使用本地执行器，暂不做服务端代理？Secret 是否必须在首版持久绑定环境，还是可先限定为安全的执行时凭据？
