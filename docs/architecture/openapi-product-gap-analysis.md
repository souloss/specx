# OpenAPI 产品能力对标与验收矩阵

状态：持续实施。此矩阵以 HTTP/OpenAPI 的日常工作流为范围，逐项对照 Apifox 与 Hoppscotch 的公开能力和当前 SpecX 行为。它是实现清单，不代表未完成能力已上线。

## 对标来源

- Apifox 文档分享、版本发布与读者入口：<https://docs.apifox.com/share-api-documentation.md>、<https://docs.apifox.com/publish-api-version.md>、<https://docs.apifox.com/view-api-documentation.md>；Hoppscotch 本地参考 `collections/documentation/PublishDocSnapshotPreview.vue` 和 backend `published-docs/input-type.args.ts`，借鉴发布前审核与快照独立性。
- Apifox 官方产品页：<https://apifox.com/>。明确介绍 OpenAPI 导入、可视化 API 设计与调试、自动化测试、数据驱动、Mock、在线文档发布和分享。
- Hoppscotch 功能清单：[`refs/hoppscotch/README.md`](../../refs/hoppscotch/README.md)。列出请求编辑、认证、环境、集合、历史、脚本、团队同步、Mock/Proxy 和 GraphQL、WebSocket、SSE、Socket.IO、MQTT 等协议。
- Hoppscotch 响应处理与 jq 筛选文档：<https://docs.hoppscotch.io/documentation/getting-started/rest/response-handling>。
- Hoppscotch 环境文档：<https://docs.hoppscotch.io/documentation/features/environments>。
- ApiCat 公共模型导航与编辑实现：`refs/apicat/frontend/apps/website/src/views/schema/SchemaPage.vue` 与 `refs/apicat/frontend/apps/website/src/layouts/ProjectDetailLayout/components/DefinitionSchemaTree/DefinitionSchemaTree.vue`；借鉴模型独立导航、字段视图及使用影响，SpecX 沿用明确保存和规范 ETag。
- Hoppscotch 集合文档：<https://docs.hoppscotch.io/documentation/features/collections>。
- Hoppscotch 脚本文档：<https://docs.hoppscotch.io/documentation/features/scripts>。
- Apifox 请求发送与响应提取：<https://docs.apifox.com/send-request.md>；Hoppscotch 本地参考：`JSONLensRenderer.vue` 和 `SaveResponseName.vue`。
- Apifox 工作台参考：本轮用户提供的 Apifox 接口工作台截图 `image.png`，用于检查导航、请求编辑、响应、测试和 Mock 的信息组织。

外部产品会持续变化。实现每个阶段时需重新核对公开文档、参考代码及 SpecX 实际界面，不能把营销页或单元测试当作端到端验收。

## 验收范围

首要闭环是：导入或同步 OpenAPI → 按 Tag/操作浏览 → 阅读完整契约 → 选择环境和认证 → 编辑请求 → 发送 → 检查响应 → 保存并复用 → 在集合中运行测试 → 发布文档或 Mock。团队共享和跨设备恢复是服务端平台能力，不应以浏览器 localStorage 代替。

| 工作流 | Apifox / Hoppscotch 对标行为 | SpecX 当前已验证行为 | 缺口与完成标准 | 状态 |
| --- | --- | --- | --- | --- |
| OpenAPI 导入和版本 | 导入 OpenAPI/Swagger；持续查看接口、模型及版本变更 | 本地文件与仓库来源、手动同步、修订历史和结构化 diff 已接入 | 增加 Apifox/Postman/Hoppscotch 集合迁移不应覆盖规范来源；导入失败要逐项报告并可重试 | 部分 |
| API 导航与文档 | 按目录/Tag/操作定位，查看参数、请求体、响应、模型和引用字段 | 服务分类、Tag、操作投影；操作页按“调试工作台 / 设计 / 文档预览”分成独立整页 Tab；本地 `$ref`、`allOf`、`oneOf` 和递归字段信息可查看；Schema 默认展开字段表，展示必填、类型、说明、约束、示例和本地引用路径，可展开/复制解析后的 JSON；文档请求/响应示例支持命名选择、说明、复制与 Schema 生成；在 5176 的真实导入规范中验证了 `POST /users` 的 `ErrorResponse` 和 `GlobalCredential` 原始字段，以及桌面/390px 视口下 Operation 叶子、三个 Tab 切换与无横向溢出；窄屏选择资源后自动收起侧边栏 | 已接入公共模型目录、名称/字段搜索、引用跳转、直接/间接使用位置和循环展开边界；仍需验证数千操作/模型下的索引和虚拟滚动性能 | 部分 |
| API 设计编辑 | 在单个 API 视图中编辑元信息、参数和请求/响应契约 | 规范根节点可新增带 method/path/摘要/Tag 的接口，失败保留输入并支持重试；独立设计 Tab 可编辑当前 Operation 的 method/path、摘要、Operation ID、标签、说明、参数和请求/响应 Schema；地址修改同步迁移参数、继承服务器及本地引用，并更新现有标签页与侧栏；重复路由、等价模板和引用型路径禁止保存；支持参数与响应增删、请求体增删、请求/响应媒体类型增删、对象属性递归维护、必填、枚举、格式、默认值、字符串长度与模式、数值范围、数组项数/唯一性、对象属性数和字段说明；新增媒体类型生成对应 Schema 草稿，删除媒体类型同步移除其 Schema；请求体至少保留一种媒体类型，204/304/1xx 响应不可添加响应体；路径级继承参数只读，避免改动同路径的其他 Operation；修改共享本地 Schema 时仅在当前 Operation 内生成独立 Schema，公共模型和其它接口保持不变；只改元信息时保留引用；设计草稿按工作区、规范和 Operation 隔离，在三个 Tab、切换接口和刷新后恢复；非法的未完成字段也保留；接口/模型更新时阻止覆盖并支持下载草稿和重置；oneOf/anyOf/allOf 分支、嵌套本地引用、nullable/readOnly/writeOnly/deprecated、exclusive bounds、additionalProperties 和参数 style/explode/allowReserved 已接入；外部/循环/缺失引用及条件/动态 Schema 的字段仍只读，可整体替换为本地模型；响应状态改名保留完整响应，1xx/204/304 清除正文；字段高级设置默认收起并显示已配置数量，保存工具栏随滚动保留；保存复用规范修订和 ETag 链路，组件测试覆盖 Operation 隔离、媒体类型和 Schema 约束保存与引用边界 | 跨设备设计草稿、冲突合并、保存前兼容性预览、共享路径模板协同改名和批量设计 | 部分 |
| 公共数据模型 | 独立模型目录、字段设计、引用复用、影响分析和生命周期 | 从 `components.schemas` 投影模型，兼容发现 `definitions`；独立预览/设计页；字段与 JSON/YAML 编辑、格式化、模型新增/改名/受保护删除；本地引用和 discriminator 映射改名同步；模型与接口双向跳转；请求/响应/参数/嵌套字段可通过搜索和预览选择模型引用；公共编辑、保持引用与局部展开明确区分，显式展开可独立保存；复用 ETag/修订并保留设备草稿 | 跨规范引用/共享模型库、兼容性分析与可视化冲突合并尚未闭环 | 部分 |
| 请求编辑 | Path、Query、Header、Cookie、认证、Body，行启停与批量编辑 | 行编辑、JSON/form/raw body、环境变量、服务器变量、认证提示和参数启停；有效服务器按 Operation/Path Item/根文档继承，环境和服务默认地址可覆盖；集合运行逐个使用对应接口的服务器与变量默认值；服务器 Base Path 下的路径参数替换及 URL 恢复保留前缀；Query、Header、Cookie 均支持行编辑与 `name: value` 批量编辑，`# name: value` 禁用行并保留双向状态；批量解析错误显示行号且保留输入；已保存/导入请求的显式 URL 与 Query 行状态同步，额外 Query 参数参与最终发送；Path、Query、Header、Cookie 参数按 OpenAPI `style`/`explode` 序列化，Query 支持 `allowReserved` 的安全子集，恢复 URL 时保留数组/对象边界；数组/对象参数可逐项添加、编辑、删除，按 Schema 使用布尔/数字控件并遵守条目/属性数约束；请求草稿支持未保存状态、撤销/重做；支持请求级 URL/method 覆盖，导入/cURL/独立请求可以直接发送并使用环境 Base URL；multipart binary 字段支持必填校验、单文件/多文件选择和 FormData 发送；顶层 octet-stream/binary 请求体可选文件直接发送；本地文件内容不进入已保存请求或历史；具名 OpenAPI 请求示例可在调试页选择并写入请求体；可选发送前校验必填参数、请求体/表单与 JSON Schema，失败不发送且可关闭校验进行负向测试 | 跨设备历史与草稿恢复、参数 Schema 完整约束及非 JSON Body 契约校验仍未闭环 | 部分 |
| 认证 | Basic、Bearer、API Key、OAuth 2/OIDC；请求可继承认证，敏感值可复用 | 支持按 Operation 继承或覆盖认证，区分 OR 方案与 AND 组合；Basic、Bearer、API Key、OAuth 2/OIDC 使用同一请求解析。共享请求只保存模式、方案与变量引用，集合按保存时的配置运行；实际浏览器验证环境切换、Unicode Basic、PKCE 以及迟到令牌隔离 | 继续补齐集合级共享认证、跨格式导出映射、长期令牌轮换及受限本机网络执行；Web 自定义 Cookie 仍受浏览器限制 | 部分 |
| 环境和变量 | 多环境、变量启停/敏感标记、作用域、导入导出；脚本可读写变量 | 服务级环境、Base URL、Secret、JSON 导入导出、默认环境和变量解析 | 支持共享环境优先级/覆盖规则、变量缺失提示、密钥轮换；跨设备共享已由服务端保存但工作台只读不落明文 | 部分 |
| 请求历史 | 最近请求恢复、清除和跨设备同步 | 本机历史、恢复、清空、敏感字段脱敏 | 服务端/用户私有历史与跨设备恢复、按状态/接口/时间过滤、保留策略 | 缺失 |
| 集合和请求 | 无限层级文件夹、请求排序/复用、导入导出、团队协作和跨设备同步 | 服务级集合树与请求定义已保存到工作区服务端资源；支持 SpecX、Postman v2.0/v2.1、Hoppscotch 导入预览、跳过项/警告和 Postman/Hoppscotch 导出；服务端强 ETag/CAS 防止静默覆盖，并清理敏感 Header、Cookie、URL 参数、结构化 Body 和已配置 Secret；请求历史和响应仍在本机 | Apifox 导入导出；跨端冲突合并、成员级权限和排序拖拽 | 部分 |
| cURL 和代码生成 | cURL 导入/复制，多语言代码生成 | 单请求 cURL 解析预览、具体错误与文件/网络选项提示；保留重复数据、URL 编码、JSON、Basic/Bearer、Cookie、multipart 与二进制文件；片段通过统一语言/客户端选择器提供 20 种组合，支持复制与下载，统一 Cookie、媒体类型、20 秒超时与文件元数据；既有 cURL、Fetch、Python、Go、Java 运行验收外，新增 Node.js Native/Request/Unirest、HTTPie、wget、Java OkHttp/Unirest、浏览器 XHR/jQuery 共 36 次真实请求验收，覆盖 JSON、multipart、binary 和 GET | Axios、Windows/PowerShell、PHP、Ruby、C# 等仍需对应运行时验收；完整 Windows Shell 语法与更多 cURL 选项；当前网络执行仍遵守浏览器限制 | 部分 |
| 请求脚本 | 发送前设置变量/时间戳/签名；受控沙箱 | 已支持请求级 pre-request/test 脚本编辑、Postman/Hoppscotch 常用兼容入口、隔离 Worker + QuickJS WASM、超时/内存/栈限制、请求改写、普通变量流转、响应 JSON 与 expect 断言、日志和集合报告；脚本字段随工作区集合保存并参与导入导出 | 继续补齐更多 Chai/Hoppscotch 断言语法、脚本调试断点与服务端 CI/CLI 执行；敏感变量仍禁止进入脚本上下文 | 部分 |
| API 测试和集合运行 | 可视化编排、断言、数据驱动、步骤依赖、报告和 CI/CLI | 单次请求支持独立 Worker/Ajv 的 JSON Schema 校验（枚举、范围、格式、额外属性、组合、递归模型，2 秒超时）和状态/Headers/JSONPath/文本断言；响应按实际 status/Content-Type 选契约，截断或无法解析 Schema 不会显示通过，新响应清除旧结果；集合按服务端共享的保存顺序执行，支持停止/继续策略、所选环境与 Secret 解析、每次最多 100 行的 JSON 数据迭代、逐请求轮次/状态/耗时/断言/变量提取报告；JSONPath 可将响应值写为本轮变量并传给同一数据迭代的后续请求，缺失变量会阻止发送；摘要导出不含响应正文、变量值或失败详情，并复用单请求超时与响应大小限制 | 增加显式步骤依赖、重试、CI/CLI 和可重复的服务端报告 | 部分 |
| Mock 服务 | 从 Schema/示例生成稳定可访问的 API Mock，变更随规范同步 | 服务设置可生成/轮换并复制公开地址、停用令牌；服务端按 method/path 匹配 operation，优先使用示例并支持 bounded schema response；令牌只显示一次且数据库只存 SHA-256 hash | 支持 response 选择/覆盖、确定性丰富样例策略、访问权限配置和跨多个 OpenAPI 规范的选择；真实导入规范的 Mock URL 仍需端到端验收 | 部分 |
| 响应检查 | 状态、耗时、大小、headers、格式化 body、预览、搜索和下载 | 状态、耗时、headers、raw/body、HTML 安全预览、XML 格式化、图片预览；非文本响应以原始字节受限预览/下载，优先读取 `Content-Disposition` 文件名，并显示大小与截断状态；正文使用虚拟滚动，支持 JSON 高亮、折叠/展开、换行、文本查找与 JSONPath/源值复制；格式化保留长整数和重复键；完整 JSON 支持 jq 筛选、独立复制下载、取消及错误提示；响应最大读取 1 MiB；可审核实际响应，按状态码或媒体类型追加响应定义，预览并推断 Schema 或仅添加示例，确认后进入当前接口设计草稿；保存后在文档中查看字段和示例 | 增加可选的上下分屏 | 部分 |
| 文档发布和分享 | 发布文档快照、固定版本、公开链接和访问控制 | owner/admin 选择已保存修订，预览后发布；独立匿名只读页面按 Tag/Operation 搜索和阅读完整字段，支持链接、下载与撤销；发布不改写源规范；已知 Secret 拒绝、运行服务器移除、事务审计和托管备份链接隔离已有 | 密码/有效期/接口范围、受限分享权限、独立自定义域名配置；持续扩充大规范性能和内容审核体验 | 部分 |
| CORS 与网络执行 | Hoppscotch 提供代理；桌面客户端可发送受浏览器限制的请求 | Web 与 Tauri 目前都用浏览器 `fetch`，遵守 CORS | Web 仅在管理员 allowlist、DNS/IP 重绑定防护和审计下使用代理；Tauri 使用受限本机 executor；两者共享超时、大小和脱敏规则 | 部分 |
| 其他 API 协议 | Hoppscotch 还支持 GraphQL、WebSocket、SSE、Socket.IO、MQTT | 仅 OpenAPI/HTTP provider | 通过独立 provider 增加协议；不把这些请求类型伪装成 OpenAPI operation，也不阻塞当前 REST 闭环 | 未开始 |

## 安全与平台边界

- 规范、环境、Secret、集合和共享测试定义属于服务器资源；单人草稿、临时响应和未保存请求可以是设备侧状态。
- Secret 明文只在用户显式执行时通过最小权限接口解析；永不写入历史、收藏导出、断言报告、错误消息或遥测。
- Web 请求默认受 CORS 约束。代理必须使用显式 allowlist、解析后的 IP 校验、重定向复验、出站限制、大小/超时限制和审计，不能接受任意服务端 URL。
- 本地 Mock、规范示例和真实 Mock endpoint 是不同能力；生成 JSON 不能标记为 Mock server 已完成。
- 第三方脚本不能获得 DOM、文件系统、平台身份、Secret 或任意网络访问；脚本能力需独立威胁建模和沙箱验证。

## 验收规则

每一项“已完成”都要同时有纯逻辑测试、权限/契约测试（涉及服务端时）、真实 UI 操作和可复现结果。自动化验证至少覆盖导入的真实 OpenAPI、环境切换、带认证请求、GET/POST、响应断言、集合恢复与运行、Mock URL、文档分享、错误/取消/超时、浅色/深色和窄窗口。外部参考应用不能代替 SpecX 自身验收，截图不能代替交互。

## 本轮 Operation 工作流验收

2026-11-11 使用最新前端生产构建与已重新编译的 `specx-web` 启动独立验收实例。数据来自 5176 工作区的 SQLite 备份，使用其已导入的 **Pet Store API**；所有写入都发生在临时副本中，原工作区数据不变。

- 从提供者目录、规范和 Tag 展开到 `GET /pets/{id}`、`POST /pets` 叶子；文档只展示选中的操作，并能查看引用模型的原始字段。
- 设计草稿在三个本地 Tab、切换 GET/POST 和刷新后分别恢复；实际通过 HTTP 保存摘要、操作级参数说明、响应字段和请求字段。两次界面保存恰好新增两条修订，刷新后内容仍在。Operation 页没有整份规范的编辑、删除工具栏。
- 使用独立写入方新增一条修订，再从旧 ETag 的界面保存，实际收到 412；界面刷新后保留本地草稿、显示冲突并禁用覆盖。草稿下载、冲突状态刷新恢复和显式放弃草稿后加载最新定义均经浏览器验证。原文编辑草稿不受接口保存影响；刷新后标签页保持 Operation 名称。
- 对保存后的整份规范做语义比较：只有对应 Operation 的预期字段变化；路径级继承参数、共享 `components`、未编辑的引用和其它操作保持不变。共享 Schema 的编辑结果仅在所选 Operation 内展开。
- 调试工作台实际向受控本地 HTTP 服务发送 GET 和 POST，核对 URL、JSON 请求体、200/201 状态与响应展示；收到响应后不再显示待发送空提示。
- 实际维护 allOf 中的嵌套引用字段、readOnly/nullable、minLength、必填 Query 和响应状态码；编辑均只写入当前 Operation。真实发送前校验阻止缺失 trace 和 minLength 失败，修正后发送成功；生产 Worker 对合法响应显示通过，对不满足设计约束的响应报告字段路径与 minLength，且新请求清除旧结果。切换 Operation/更新规范时取消未完成的请求并忽略迟到响应；显式取消与超时有不同反馈。
- 按 `web-design-guidelines` 检查后，字段高级选项折叠到原生 details，常用字段保持紧凑；真实滚动验证保存入口保持可见。
- 实际切换浅色/深色主题检查三个 Tab；验证方向键、Home/End 切换与焦点同步；390px 下选择叶子自动收起侧栏，三个页面没有页面级横向溢出。浏览器无未处理异常。

可复现脚本、结果 JSON 与截图保存在本地忽略目录 `outputs/openapi-operation-acceptance/`。前端全量 292 项测试、类型检查、生产构建、lint、格式与开发入口 CSS 检查通过；本轮此前完成的 Rust 工作区/Tauri 检查与迁移检查另有执行记录。未配置 PostgreSQL 测试连接，本轮没有验证真实 PostgreSQL 数据库。

此次结果只覆盖以上工作流；跨设备的未保存草稿恢复、自动冲突合并、共享路径模板协同改名以及表中标记的其它产品缺口仍未闭环，不能据此宣称已达到两款完整产品的全部核心能力。


## Operation 创建、地址编辑与恢复补充验收

同日使用最新生产构建和 5176 数据的独立 SQLite 副本完成 `outputs/openapi-operation-lifecycle/verify.py`：

- 在规范根节点创建 `POST /orders/{orderId}`，选择方法和填写摘要/Tag；重复路由无写入，模拟断网后保留表单，重试只新增一条修订。新 Tag 与叶子即时出现，文档仅显示该接口且生成必填路径参数。
- 在设计页将该操作改为 `PATCH /purchases/{purchaseId}`，一并保存摘要；侧栏与原标签页身份更新，旧地址不再出现，刷新后没有错误恢复旧地址草稿。
- 将已导入的 `GET /pets/{id}` 移到 `PATCH /animals/{petId}`；逐字段比较整份规范，确认相邻接口、共享模型及原路径元信息不变，继承参数与服务器被保留。向受控 HTTP 服务实际发送 `PATCH /purchases/19` 和带服务器变量前缀的 `PATCH /v9/animals/42`，方法和路径均正确。
- 三次界面保存恰好新增三条修订；历史界面选择基线修订，进入单弹窗确认、通过键盘定位取消、取消返回原按钮后重新确认恢复。恢复恰好新增一条修订，OpenAPI 类型、原叶子导航与文档视图保持正确。
- 桌面浅色/深色、390px 三个 Tab、新建弹窗、字段编辑、键盘 Tab 导航均经实际操作；新建弹窗页脚位于弹窗边界内，无页面级横向溢出、无浏览器未处理异常。原工作区数据未写入。

单元/组件测试另覆盖服务器优先级、不同 Operation 集合执行的服务器变量隔离、保留未改名占位符后再匹配新名称、移动后本地引用解析、Markdown/OpenAPI 同步及恢复的类型保留。前端类型检查、生产构建、lint、格式、开发入口 CSS 与 `git diff --check` 通过。Vite 仍提示主包体积较大；本轮未新增 Rust/API/数据库契约变更，也未执行真实 PostgreSQL 测试。

## 公共模型工作流补充验收

2026-11-11 使用最新生产构建与 `specx-web`，在 5176 数据的独立 SQLite 副本执行 `outputs/openapi-models/verify.py`：

- 模型目录按字段搜索，文档 → 模型 → 引用模型/使用接口双向导航；`allOf` 模型展示合并后的原始字段，同时能跳到被引用模型。
- 可视编辑 `NewPet` 公共字段，模拟断网后保留输入并重试；改名为 `PetInput` 后更新请求体、模型组合和已打开标签，刷新后身份保持正确。整份规范逐字段比较，未涉及的接口和公共组件保持不变；示例和扩展中的 `$ref` 字面值不改写。
- 重复模型禁止创建；创建 `AuditEvent` 后使用实际 CodeMirror 编辑 JSON/YAML、转换和格式化。生产 Worker 阻止 `type: 42` 的无效 Schema 且不写入；修正后成功保存字段并在预览展示。被引用模型删除受阻，未使用模型经确认后删除并返回目录。
- 5 次成功界面操作恰好新增 5 条修订。独立写入方修改同一模型后，界面实际收到 412；草稿、冲突提示和禁止覆盖状态在刷新后仍保持，可下载再明确放弃并加载远端定义。
- 递归 `Node` 的两个自引用只产生 3 个预览节点，保留跳转与递归提示。键盘方向键/Home 切换模型 Tab；保存工具栏可随滚动保持可见；桌面浅色/深色及 390px 预览、设计、源码编辑无页面横向溢出，无浏览器未处理异常。

当前工作区的原始数据未写入。此次另回归 `outputs/openapi-operation-acceptance/verify.py` 与 `outputs/openapi-operation-lifecycle/verify.py`，覆盖真实 GET/POST/PATCH、接口设计隔离、创建/地址移动、历史恢复与三 Tab 的样式交互。全量前端 307 项测试（41 个文件）、类型检查、生产构建、lint、格式、5176 开发 CSS 与 `git diff --check` 均通过；本阶段没有新增 Rust/API/数据库契约，也未执行真实 PostgreSQL 测试。大型主包的构建提示仍存在。

本轮补齐公共模型日常维护，不代表已覆盖完整 Apifox/Hoppscotch；模型引用选择器的后续实现与验收见下一节，跨设备草稿和服务端测试执行等仍按上表继续实施。

## 设计时模型复用补充验收

本阶段借鉴模型库和请求工作台分离的组织方式，将已有公共模型接入 Operation 设计，而不引入另一套保存、身份或并发机制。请求、响应、参数和嵌套字段使用同一模型选择器；公共模型也能组合引用其他模型。选择器支持按字段搜索、完整预览、键盘单选和明确确认；选择引用、保持引用和展开为独立结构具有不同的持久化语义。

验收使用最新生产前端、现有 `specx-web` 与 5176 工作区的隔离 SQLite 备份，脚本、结果 JSON 与截图位于本地忽略目录 `outputs/openapi-model-reuse/`：

- 取消和再次选择当前引用不产生修改；只编辑元信息保留原有引用及 3.1 附加约束。更换引用保留附加约束；单独执行“展开为独立结构”即可保存，刷新后仍是独立结构，附加规则按交集保留。
- 请求、响应和嵌套字段选择模型后，可跨 Tab 和刷新恢复；模拟断网保留选择，重试成功后只有一条新修订。完整规范逐字段比较，确认共享模型、相邻接口与未编辑参数保持不变。
- 参数结构复用公共模型并实际保存；公共模型字段更新后，接口文档中保持引用的部分同步展示，而已展开的局部响应说明独立保留。
- 8 次成功界面保存恰好新增 8 条修订。独立写入方修改当前接口后，旧界面保存实际收到 412；选择的模型引用跨刷新保留，可下载草稿，覆盖按钮禁用，显式放弃后加载最新定义。
- 搜索按字段过滤，完整模型预览支持组合字段和布尔模型；原生单选控件经键盘 Space/方向键操作，Escape 关闭后焦点返回原选择器。桌面浅色/深色与 390px 弹窗均实际操作，确认页脚位于弹窗内，没有页面级横向溢出或浏览器未处理异常。

最终构建同时回归公共模型管理、Operation 设计/真实 GET 与 POST、Operation 创建/地址迁移/真实 PATCH/历史恢复三套生产验收。前端全量 318 项测试（43 个文件）、类型检查、生产构建、lint、格式、5176 开发 CSS 与 `git diff --check` 通过。选择器单独按需加载；主包仍约 1.74 MB，有构建体积提示。本阶段没有新增 Rust/API/数据库契约，未运行真实 PostgreSQL 测试，也没有更改 5176 原始工作区数据。

该结果仅闭环规范内的模型复用。跨规范模型库、参数 `content`/直接布尔 Schema 的可视设计、跨设备草稿、兼容性预览和服务端测试执行仍在矩阵中保留缺口。


## 调试请求导入与代码生成补充验收

2026-11-11 对照 Hoppscotch 的 cURL 解析与中间请求模型，补齐导入预览及生成代码的实际请求语义。没有复制其组件布局，也没有在浏览器执行用户粘贴的命令。

- 使用最新生产构建与 `specx-web`，在 5176 数据的独立 SQLite 备份运行 `outputs/openapi-curl/verify.py`。聚焦 POST Operation 时导入普通/带查询的 GET，实际发送的方法正确；未知选项明确报错且原编辑器不变。带百分号、中文、引号和反斜杠的 JSON 按原字节发送；URL 编码正文保留重复字段、空格与加号。
- multipart 和二进制导入都先要求用户选择本地文件；实际接收重复文本字段、文件名、MIME 与原始字节。二进制模式参与撤销/重做，切到其他接口不残留。调试导入不会改动设计/文档 Tab 或规范正文/版本。
- 下载 Node Fetch 片段并实际执行，确认收到与浏览器发送相同的正文；剪贴板内容与预览一致，代码与响应各自显示复制反馈。浅色/深色和 390px 预览、全宽命令输入、键盘焦点与页面宽度均检查；没有浏览器未处理异常。
- `outputs/openapi-curl/codegen_runtime.py` 实际运行 cURL、Node Fetch、Python requests、Go、Java 的 JSON/multipart/binary/GET，共 20 组请求。按接收端验证方法、Cookie、请求头、UTF-8 正文、重复字段与文件元数据；文件源路径包含逗号、分号和引号。修正了 Java 对 JSON Unicode 转义的误写和 cURL 对 MIME 类型的错误引号，并增加可执行回归测试；Node 片段另验证超时和空 multipart。
- 回归 Operation 设计、Operation 创建/迁移/历史恢复、公共模型管理、模型引用复用四套浏览器验收，涵盖真实请求、412、断网草稿、修改隔离和窄窗口。前端 356 项测试（45 个文件）、类型检查、生产构建、lint、格式、5176 开发 CSS 和 `git diff --check` 通过。

Axios 依赖及 PHP、Ruby、C# 运行时未在本机提供，本轮未执行这些语言的生成结果。Web 的 Cookie/受限 Header/CORS 仍遵守浏览器行为；CLI 网络选项不等同于浏览器选项，界面明确提示。主包约 1.76 MB 的体积提示仍存在。本阶段没有 Rust/API/数据库契约变更，也没有对 PostgreSQL 或原工作区做写入验证；代码保持未提交。共享授权、跨设备草稿、服务端执行与矩阵中的其他缺口继续保留，完整产品对标仍未闭环。


## 认证与环境补充验收

对照 Hoppscotch 的 `Authorization.vue` 与 `EffectiveURL.ts`，复用“先构造实际请求，再执行”的思路，按 OpenAPI 语义实现可选方案和组合方案。调试界面直接显示选中的组合、当前环境中可用的变量名和缺失凭据；提交失败时打开认证区并聚焦对应输入。认证不会改写规范定义。

- 使用 5176 的已导入 **Pet Store API** 的隔离 SQLite 备份，启动最新生产前端和重新编译的 `specx-web`，通过 Playwright 实际发送 11 个请求。核对 AND 的自定义 Header/Query，切换 Unicode Basic 和匿名 OR 方案，以及为独立导入的请求指定认证。拒绝缺失凭据的发送，并聚焦待填写字段。
- 保存后在清空本地存储的浏览器上下文恢复模式和变量引用。切换两个环境实际发送各自的加密 Secret；集合在页面选择“无需认证”时仍按每条保存配置发送。API、浏览器本地历史及代码片段均验证没有保存已解析的明文凭据；历史标题也一并检查。
- 本地 OAuth 服务完成真实 PKCE 跳转和换取令牌；同一环境可使用令牌，环境切换后停止发送旧令牌，延迟返回的令牌也不会应用。桌面浅色、深色与 390px 验证键盘、焦点和宽度，无浏览器未处理异常。
- 前端 370 项测试、生产构建、lint、格式、5176 开发 CSS 检查，Rust workspace 与 Tauri 测试、API OpenAPI 契约、clippy、迁移检查及 `git diff --check` 通过。生产构建同时回归 Operation 三 Tab 和 cURL 导入。浏览器脚本、结果和截图在本地忽略目录 `outputs/openapi-auth/`。

本轮新增可选 API 字段，复用已有请求 JSON 持久化，无新增迁移；未执行真实 PostgreSQL 或原生客户端界面验收。服务端集合执行、集合级认证和非 HTTP 协议仍在对标矩阵中，完整产品对标尚未闭环。

## 响应查看与示例补充验收

2026-11-11 使用最新生产构建与 `specx-web`，在 5176 工作区的独立 SQLite 备份上通过 Chromium 验收。借鉴 Apifox 将实测响应提取到接口定义的流程和 Hoppscotch 的正文查找与具名示例；SpecX 先将用户审核后的正文写入当前 Operation 的设计草稿，再由显式保存产生规范修订。

- 发送 6 次真实请求。300,400 字节的 JSON 初始只渲染 36 行；展开、折叠和查找可定位第 4,500 条记录。原始正文复制和下载保留长整数、重复键和转义；响应头与原始 HTTP 复制遵循当前视图。验证纯文本、超过 1 MiB 的截断提示与文本回退。
- 提取弹窗拒绝会丢值的 JSON，取消返回触发按钮。审核后的具名示例跨刷新恢复；保存恰好新增一条修订，保留既有示例、Schema、公共组件与相邻 Operation。文档预览可选择新示例；从设计切到文档时顶部 Tab 位置不跳动。
- 验证桌面浅色、深色和 390 px 窄窗口，检查键盘焦点、弹窗操作区和页面宽度，无浏览器未处理异常。以最终生产构建回归 Operation 三 Tab、真实 GET/POST/PATCH、接口移动与修订恢复；本轮也回归认证及 cURL 导入。

验收脚本、截图和结果位于本地 `outputs/openapi-response/`。前端 376 项测试、Rust 工作区与 Tauri 测试、生产构建、lint、格式、5176 开发 CSS、迁移检查与 `git diff --check` 通过。未写入 5176 原始工作区，也未执行真实 PostgreSQL 或原生客户端界面验收。

随后对照 Hoppscotch 的官方 jq 操作说明和 `refs/hoppscotch` 中的响应查看器补齐结构筛选。使用最终生产构建在 Chromium 实际运行管道、`select`、对象投影和多值输出，验证精确数字、剪贴板、JSON 下载、空结果、语法错误、超时、取消、大小和数量限制。首次执行才从同源加载 jq；切到响应头或原始 HTTP 后仍能返回同一筛选结果，契约校验继续使用完整正文。以 300,400 字节的响应实际筛到第 4,500 条记录；回归浅色、深色、390 px、接口三个 Tab 和示例提取。前端 378 项测试、Rust 工作区与 Tauri 测试、生产构建、lint、格式和 5176 开发 CSS 均通过。Hoppscotch 托管版在本轮未成功加载到请求编辑器，筛选对照基于公开文档、参考源码和实际 jq 执行。本轮筛选验收不覆盖文档发布及服务端集合执行。

继续按 [Apifox 响应提取](https://docs.apifox.com/extract-response-example.md)区分结构和示例，补齐从实测响应追加状态码或媒体类型。使用 5176 工作区备份中的 Pet Store API，在最新生产构建中发送真实 HTTP 请求：`422 application/problem+json` 可审核正文并预览具体字段，将结构和示例加入设计草稿；`200 text/plain` 可仅追加示例。校验了刷新后恢复、冲突不覆盖、每次显式保存只新增一条修订，以及文档预览显示字段和示例。逐项比对确认保留既有 `200` JSON 契约、共享 `418` / `4XX` 引用与相邻接口。Chromium 完成浅色、深色、390 px、键盘取消和 jq 回归，前端 384 项测试及仓库构建和测试通过。验收脚本、报告和截图在本地 `outputs/openapi-response-capture/`。

## 固定修订文档分享验收

2026-11-12 使用最新 Vite 生产构建、重新编译的 `specx-web` 和 5176 Pet Store API 的隔离 SQLite 副本执行 `outputs/openapi-publication/verify.py`；新增 PostgreSQL 迁移与持久化测试使用隔离 PostgreSQL 17 容器。

- 管理员可从已保存修订进入“预览并分享”，预览只展示所选修订的 Tag、Operation、引用模型字段和示例；切换旧修订不会读取当前草稿，取消不会产生分享记录，重复发布同一修订会禁用提交。
- 发布快照移除根、路径、操作和回调路径的运行服务器地址，保留 Schema 中名为 `servers` 的字段；标题或正文出现已知 Secret 时返回 `422`。发布不修改规范 ETag 或新增规范修订，审计与摘要校验在服务端事务中完成。
- 匿名读者只请求专用公开 OpenAPI API，支持搜索、Tag/Operation 目录、键盘进入接口、片段链接、刷新恢复、下载和撤销后的友好错误；不加载工作区、环境、Secret 或设计接口，在线调试按发布选项单独开放。公开响应 `no-store`，页面禁止 referrer、索引和嵌入。
- 实际验证了已发布链接与候选修订互不影响、伪造租户/操作者请求返回 `401`、取消撤销不会失效、撤销后返回 `404`、另一版本仍可阅读。桌面浅色、深色和 390px 窄窗口的读者页面与管理预览均无页面级横向溢出；浏览器无未处理异常。
- 前端 395 项测试、API 45 项、实际 PostgreSQL Core 10 项（含发布跨客户端持久化/隔离/级联）、Tauri 4 项、lint、类型检查、生产构建、Rust clippy、迁移检查和 `git diff --check` 通过。验收截图和结果 JSON 位于 `outputs/openapi-publication/`；原工作区未写入。

本阶段已闭环“固定修订的公开只读文档分享”，但不代表完整 Apifox/Hoppscotch 对标完成。矩阵中仍保留密码或有效期分享、接口范围筛选、受限分享权限、独立自定义域名配置、跨设备草稿、服务端集合执行、兼容性分析和非 HTTP 协议等工作。


### 公开文档站与在线调试补充验收

2026-11-12 再次使用 Chromium 实际访问 `https://a4l1x4y0ul.apifox.cn/`，核对其 Tag/Operation 目录、鉴权说明、字段、模型与请求示例，吸收这些信息组织方式。实现保持 SpecX 的独立阅读模块，并复用内部接口文档及浏览器请求工作台。

使用重新编译的 Web Host 与最新生产前端、5176 Pet Store API 的隔离 SQLite 副本，验证发布标题、说明、主题、下载和在线调试选项，发布前预览、取消发布、不同修订的随机子域名、复制、撤销及重新发布。公开页面的搜索、片段链接刷新、返回/前进、模型字段/依赖/使用位置、多语言请求代码、剪贴板和下载均执行真实交互。工作台在首次进入调试时加载，文档阅读不加载其代码；公开请求不读取工作区 API。

受控 CORS HTTP 服务实际收到 GET `/pets?limit=3` 和 POST JSON，核对独立输入的 Bearer 凭据与无 Cookie 请求。切换文档/调试保留当前请求，切换接口不混用请求。浅色、深色、390px 页面和发布弹窗均检查无页面级横向溢出；关闭下载和调试后入口消失。取消撤销保留访问，确认撤销后 API 返回 404、页面显示友好空状态，其他链接及源修订保持不变。验收结果与截图位于本机忽略目录 `outputs/openapi-publication-final/`。

本机随机域名使用 `.localhost:4349`；生产随机子域名仍需部署 wildcard DNS 与 TLS，平台不负责注册公网域名。Vite 的共享 client chunk 仍有体积提示，公开工作台已按需加载；后续可继续按共享模块使用情况拆包。

### 阅读模块与示例联动验收

2026-11-12 继续实际检查参考站的鉴权、参数、响应和代码示例组织。将接口文档拆成鉴权、参数、响应、媒体内容和 Schema 等独立组件，由文档容器管理选择状态，内部文档和公开站复用。宽屏分栏显示契约与请求/响应示例；参数明确区分示例和默认值，并显示嵌套字段格式、排他边界和对象/数组约束。单页 Markdown 包含有效鉴权要求及相关定义。请求正文媒体类型和具名示例切换会更新生成代码，响应状态/媒体类型在结构与示例之间同步。剩余业务下拉统一接入 `AppSelect`，保留主题适配与键盘语义。

使用最新生产构建、已编译的 Web Host 与隔离 SQLite 副本执行 `outputs/openapi-parity/acceptance.py`。实际发布两个修订，验收配置预览、取消、独立随机域名、匿名阅读、搜索、深链接、模型字段/示例、剪贴板、下载、状态/媒体/具名示例联动和下拉键盘操作。受控 CORS 服务收到真实 GET 查询与 POST JSON，确认显式 Bearer 凭据和不携带 Cookie；文档/调试切换保留请求，切换接口隔离输入。关闭下载/调试、取消撤销、确认撤销后的 404、其他发布仍可读取以及源 ETag 不变均通过。检查浅色、深色和 390px 截图，无页面级横向溢出或浏览器未处理异常。

本轮前端 409 项测试（57 个文件）、类型检查、生产构建、lint、格式、迁移一致性和 `git diff --check` 通过；未新增 Rust/API/数据库变更，沿用上一轮 API 45 项及真实 PostgreSQL 发布持久化验收结果。参考站调试面板在本次观察中仍处于加载状态，完整调试行为以 SpecX 自身的真实请求验收为准。截图、参考站观察记录和机器结果保存在本地忽略目录 `outputs/openapi-parity/`，未写入原工作区。生产域名配置、密码/有效期分享和其他对标缺口继续保持前述边界。

### 真实参考文档、导出与模型代码生成验收

2026-11-12 将参考站公开导出的约 839 KiB OpenAPI 导入隔离工作区，实际验证 129 个路径、286 个模型的阅读站。`ErrorResponse` 展示 `code/message/details/requestId`，`GlobalCredential` 展示 10 个原始字段及约束。模型代码生成封装为独立 Worker，文档导出、页面操作菜单和相关弹窗分别由独立模块负责，公开阅读不依赖工作区环境。

执行 `outputs/openapi-parity/extended-acceptance.py`，13 种目标语言均在浏览器内生成代码，验证 TypeScript 配置、非法名称恢复、复制、文件下载、取消及焦点返回。实际下载并校验完整和当前接口的 OpenAPI JSON、当前接口 YAML、Markdown、离线 HTML 与 LLMs.txt；当前接口导出保留本地模型和鉴权引用闭包，排除其他接口。完整 Markdown/HTML 分别约 2.49/2.55 MB，本机实测各格式导出耗时约 0.05–0.22 秒。

浅色/深色持久化、390px 深链接、目录收起、模型生成和导出弹窗均通过实际交互及截图检查；关闭下载后，定义、示例、请求片段、模型代码和文档导出入口均隐藏，匿名页面只请求公开 API。结果保存在 `outputs/openapi-parity/extended-result.json`。前端 423 项测试、生产构建、lint、格式检查通过。随机公网域名仍需要部署通配 DNS、反向代理及 TLS；本阶段没有自动配置这些基础设施。

### 阅读搜索、请求客户端与主题弹窗补充验收

2026-11-13 使用最新前端生产构建及同一隔离验收实例，继续验证参考站导出的 129 个路径、172 个接口、286 个模型。搜索索引覆盖参数和引用模型字段，支持多个关键词共同匹配，并避免把任意示例正文当作索引内容。顶部搜索按钮和共享快捷键可定位搜索框；窄屏同时展开目录。Tag 概览展示完整接口列表，切换目录保留搜索词，浏览器返回、前进与内容标题焦点均验证通过。概览按规范实际声明展示版本、维护者、许可证及外部文档。

请求代码由独立客户端注册表、请求模型和生成模块负责，统一提供 20 种语言/客户端组合。实际浏览器逐一切换、复制、下载，并核对代码和文件内容一致。另在真实运行时向受控 HTTP 服务发送 36 次请求：Node.js Native/Request/Unirest、HTTPie、wget 共 20 次，编译执行 Java OkHttp 4.12/Unirest 4.4.7 共 8 次，Chromium XHR/jQuery 共 8 次。每种均覆盖 JSON、multipart、binary 与 GET，核对 UTF-8、文件和重复字段；浏览器请求确认不携带跨域 Cookie。这不等于所有客户端的运行时均已验收，Windows/PowerShell、PHP、Ruby、C# 等仍保留矩阵中的运行边界。

发布预览通过独立外观作用域及浮层挂载上下文展示所选主题，不改变工作区主题。实际交叉验证浅色工作区中的深色预览、深色工作区中的浅色预览，以及下拉菜单、页面菜单、Markdown 和模型代码弹窗；关闭嵌套弹窗后焦点返回原按钮。业务下拉统一使用 `AppSelect`。浏览器量测请求客户端控件均为 32px 高、12px 字号和 4px 圆角。

窄屏验收发现并修复了模型代码超出弹窗底部、以及小高度窗口中设置与输出争用滚动空间的问题。窄屏设置和输出在弹窗内滚动，宽屏两栏分别滚动，标题与关闭按钮保持可见；代码预览随视口限制高度。浅色、深色下分别验证 390×844、390×600、844×390 和 1512×982，真实滚轮、键盘到达代码末尾、复制、下载、关闭和焦点返回均通过，并检查实际截图。

最终生产构建执行 `reader-interaction-acceptance.py`、`model-scroll-acceptance.py` 和 `extended-acceptance.py`，覆盖 5 组阅读故事、8 组窗口/主题组合及 6 组真实大文档故事，均无浏览器未处理异常。发布配置、真实 GET/POST、取消、撤销、重新发布、受限入口与源 ETag 不变另由 `acceptance.py` 验证。前端全量 431 项测试（63 个文件）、类型检查、生产构建、lint、格式与 `git diff --check` 通过；本阶段没有新增 Rust/API/数据库变更，沿用前述后端及实库发布验收。结果 JSON、截图和脚本位于本机忽略目录 `outputs/openapi-parity/`。

当前仍使用随机 `.localhost:4349` 子域名；公网访问需要部署通配 DNS、反向代理及 TLS。构建保留大于 500 KiB 的共享 chunk 体积提示。密码/有效期/接口范围分享、产品专属客户端导入入口、服务端网络执行及矩阵中的其它平台能力不在本次阅读站验收范围内。

### 公开环境、导出回导与工作台空间补充验收

2026-11-13 使用重新编译的 Web Host、最新生产前端和独立 SQLite 数据目录，完成 JSONPlaceholder 的文件导入、发布配置、匿名调试、下载和重新导入。复制服务环境时只带出已启用的普通变量，敏感值与停用变量不进入公开快照；无值域名变量阻止发布，手动放入已知 Secret 值的请求被服务端拒绝。公开环境写入标准 `servers` 和 `x-specx-public-environments`，不修改源规范修订。

匿名文档站实际发送 GET `/posts?userId=1`、修改变量后的 GET 和 POST `/posts`，分别收到 200 和 201。切换环境、恢复临时变量与重置均保持发布快照不变。使用页面下载控件导出完整 JSON 和单接口 YAML，再经文件导入向导创建规范，两者都能直接选择公开环境并发送有数据的 GET 请求。匿名页面不调用工作区接口，也不向 JSONPlaceholder 携带 Cookie。

本轮修复了公开长变量名撑出输入区域，以及 768px 窗口中侧栏挤压工作台两栏的问题。工作台按自身可用宽度排列请求和响应；发布配置中的变量在窄屏分行编辑。Chromium 验证 1512px、768px、390px 和 320px，检查实际字段宽度和截图，覆盖浅色、深色、中英文，以及媒体类型预设和自定义值。页面级与变量卡片内均无横向溢出。

前端 64 个文件、439 项测试，类型检查、生产构建、lint、格式、开发 CSS 和 `git diff --check` 通过。结果、运行脚本和截图位于忽略目录 `outputs/openapi-usability/`；本轮没有新增 Rust/API/数据库变更。验证实例使用 `.localhost:4354`，仍需生产 DNS/TLS 才能对外访问；浏览器视口检查不替代原生 Tauri 各系统实机验收。构建的共享 chunk 体积提示继续保留。
