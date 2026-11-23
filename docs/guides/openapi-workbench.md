# OpenAPI 文档与调试工作台

接口目录中的单接口页共用“文档 / 编辑 / 调试”入口。文档使用已保存的规范，
调试页维护临时请求；修改方法、URL、参数或正文不会直接改写接口定义。

## 页面与组件

| 组件 | 职责 |
| --- | --- |
| `OpenApiOperationDocument` | 接口标题、方法/路径、标签及文档区块的组装 |
| `OpenApiParameterDocument`、`OpenApiResponseDocument` | 参数位置分组、响应契约及示例同步 |
| `OpenApiOperationTabs` | 单行页签、方向键与 Home/End 导航、页签栏内滚动 |
| `OpenApiRequestTarget` | 请求方法、URL、发送/取消操作区 |
| `OpenApiDebugParameterSection` | Query、Path、Header、Cookie 区域与批量编辑模态框 |
| `OpenApiDebugParameterTable`、`OpenApiParameterValueEditor` | 参数勾选、行内添加、可编辑类型/说明、标量及结构化值输入 |
| `OpenApiDebugBody`、`OpenApiDebugFormTable` | 正文格式、表单字段及文件选择 |
| `OpenApiScriptSequence`、`OpenApiDebugSettings` | 前置/后置脚本、超时、重定向与响应校验设置 |
| `OpenApiDebugSplitPane` | 请求/响应独立滚动、指针拖动及键盘调整分栏 |
| `OpenApiDebugResponsePanel` | 响应页签/指标标题行、正文与契约侧栏布局 |
| `OpenApiResponseMetric`、`OpenApiResponseMetrics` | 耗时、大小、网络详情浮层及可访问交互 |
| `OpenApiResponseBody`、`OpenApiResponseHeaders`、`OpenApiResponseConsole`、`OpenApiExecutedRequest` | 正文、表格、脚本日志与本次提交的请求 |
| `OpenApiResponseViewer`、`OpenApiResponseViewControls` | Pretty/Raw/Preview/Visualize、语法/编码选择、原始字节解码与安全预览 |
| `OpenApiBulkEditDialog` | 逗号/冒号模式、带引号 CSV、错误行与取消/确认事务 |
| `OpenApiRequestProxyControl`、`OpenApiRequestProxyOptions` | 底栏代理选择与环境前置 URL 的代理覆盖 |
| `OpenApiEnvironmentManager`、`OpenApiEnvironmentServers`、`EnvironmentVariablesTable` | OpenAPI 环境列表、服务前置 URL、环境变量及保存 |
| `OpenApiDesignPanel`、`OpenApiDesignMetadata`、`OpenApiDesignParameters`、`OpenApiDesignExamples`、`OpenApiDesignResponseHeaders` | 接口元信息、按位置分组的参数、请求示例与响应头编辑 |

`OpenApiPreview` 组装上述组件，沿用 workspace store 和既有 backend client；
`lib/openapiDebugFields` 管理字段类型与表单批量编辑，`lib/openapiWorkbench` 管理请求格式转换和本地历史。
代理的线协议由 backend client 承接；原生 HTTP 执行独立于组件和领域持久化。

## 交互约定

- 接口编辑中的参数、响应头和对象属性可从类型下拉选择基础类型或引用模型。
  星号修改参数或父对象的必填定义。高级设置可切换字段表单和 JSON Schema，
  枚举行的名称和说明写入 `x-apifox-enum`。OpenAPI 3.1 使用包含 `null` 的
  类型数组，3.0 使用 `nullable`。模型引用的局部约束随字段保存。
- 返回响应按状态码和名称切换。名称可单独保存为 `x-apifox-name`，说明保存为
  OpenAPI `description`。同一响应可声明多个媒体类型，选择格式后编辑该格式
  的 Schema 或正文示例。选择无响应体后可在保存前切回原格式。响应头和描述属于
  状态码对应的响应。
- 参数输入显示实际参与请求构造的示例/默认值。用户清空后的空值优先于示例，
  输入中的环境变量不被数字控件拦截。逐行与批量模式使用同一组参数状态；
  批量格式错误保留文本并标出行号。
- 全选反映当前行的选中、未选中或部分选中状态；空表禁用全选。
  添加行后焦点进入新名称字段。
- 前置和后置操作分别显示。正文选择 `none` 后不发送隐藏编辑器中的内容。
- 分栏初始请求区比例为 46%，可在 20%–85% 之间调整。方向键每次调整 5%，
  Home/End 到达边界，双击恢复初始比例；响应区保留最小高度。
- 页签保持单行。方向键切换并聚焦，Home/End 跳至首尾；选中项只滚动所属页签栏，
  不带动文档、请求或响应容器。
- 收到响应后优先选择精确状态码，其次匹配 `2XX` 等范围，最后匹配 `default`。
  用户可另选已声明的响应契约进行校验。

## Apifox 对齐依据与边界

布局参考 `https://app.apifox.com/project/860863` 中“查询宠物详情”的文档与调试页面，
以及用户提供的 DOM。参照顺序为方法/URL 操作行、请求页签、Query/Path 参数表、
上下分栏、响应标题及工具区；文档按请求参数、返回响应、请求示例的顺序排列。
运行截图和浏览器验收结果存于被忽略的本地目录 `outputs/apifox-workbench/`。
参考站登录凭据不进入源码、测试夹具或文档。

这是对既有 SpecX 应用的组件化适配，没有逐字移植 Apifox 的运行包。
目前仍有明确差异：

- 应用壳、服务目录、规范修订和鉴权数据使用 SpecX 自身模型；未声明的 Apifox
  状态、创建者、责任人和 Mock 期望不会生成假数据。
- 正文目前提供 none、form-data、x-www-form-urlencoded、JSON、XML、Text、Binary、YAML、HTML 及文档声明的媒体类型；
  GraphQL 和 MessagePack 专用编辑器尚未实现。
- Mock 文档、调试和服务概览入口已隐藏；`OpenApiMockDocument` 组件和既有服务端能力保留，待交互明确后再接入。
- 请求支持浏览器 Fetch 与 SpecX 内置云端 Agent。未提供 Apifox 桌面 Agent、团队 Vault 或模块级多前置 URL。

因此当前验收说明具体已验证的布局与行为，不将其描述为所有功能的完整 1:1 复制。

## 验证

前端检查使用 `pnpm --dir apps/desktop test`、`build`、`lint`、`format:check`、
`check:dev-styles`，并运行 `git diff --check`。
定向测试覆盖参数输入与清空、部分选中、页签键盘导航、响应契约选择、分栏调整、引用类型与隐藏请求头。
浏览器验收使用隔离数据库及本地拦截的调试响应，检查浅色/深色和 1512、844、390px
窗口下的文档、调试及批量编辑弹窗。响应区另用本机 HTTP 服务验证真实计时、压缩、
空响应、截断、错误重试和脚本输出。参考站只进行临时请求编辑和只读 GET，未保存项目数据。

## 响应区对齐与测量边界

2026-11-15 在 Apifox 示例项目中实际发送 GET、切换五个响应页签，并打开耗时、大小、
网络浮层。原测试环境指向 `127.0.0.1`，先复现其 Web 失败态，再临时请求公开只读回显地址。
源站成功态采用单行 `Body / Cookie / Header / 控制台 / 实际请求`；状态码、总耗时、
响应总大小和网络图标位于同一标题行右侧。契约校验区独立于页签，在宽窗口中作为右侧栏。
参考截图保留在本机 `/tmp/specx-apifox-reference/response-*.png`，本地验收产物位于
`outputs/apifox-workbench/response/`。没有移植源站运行包或存储参考站凭据。

- 正文以单行紧凑工具栏提供 Pretty / Raw / Preview / Visualize，配合 JSON/XML/HTML/Text
  语法与 14 种编码。切换编码始终从保留的原始响应字节解码；hex/base64 不经过有损文本转换，
  utf16 识别字节序标记。HTML 和真实 `pm.visualizer.set` 的 Handlebars 渲染结果经清洗后，
  在禁止脚本、外联与父页访问的 sandbox iframe 中显示。搜索、jq、折叠、换行使用图标入口；
  下载保存原始正文 bytes，复制采用当前显示/筛选结果。校验侧栏不放复制、下载、格式化或提取按钮。
  Header 采用名称/值表格；Cookie 解析独立的 Set-Cookie（含 HttpOnly、SameSite、过期信息）；
  控制台单独显示本次脚本输出及错误。
- 请求成功后回到 Body；失败时显示“请求异常”，仍可检查控制台与已提交的实际请求。
  新请求清除上次响应和错误指标。实际请求在提交 Fetch 前记录，因此网络失败也可诊断，
  前置脚本阻止发送时则明确尚未发送。
- 耗时浮层按准备、Socket、DNS、TCP、SSL、TTFB、下载、处理组织；总时间含准备及后置处理，
  脚本中的 responseTime 继续保留网络响应时间语义。每次请求独立监听 PerformanceObserver，
  不依赖可能已满的页面 Resource Timing 共享缓冲区，结束、失败或取消时均断开监听。
  TCP 时间排除 TLS，当前 URL、起始时刻和完成时刻共同约束记录，避免复用上次请求。
- Resource Timing 未开放时，DNS/TCP/SSL/TTFB 保持“不可用”，另外展示含连接的“等待响应头”
  和实际正文读取时间。跨域细分信息由 Timing-Allow-Origin 决定，不把被屏蔽的 0 当作测量值。
- 大小详情分请求/响应及 Header/Body；有 Resource Timing 时使用其传输/编码/解码体积。
  Resource Timing 的响应 Header 本身也是标准估算；可读请求/响应头的序列化也不是线上
  压缩或自动补齐后的完整 Header，故总量和 Header 标记 `≈`。multipart 边界和流式请求体
  不猜测；未知正文或截断响应标记 `*` 并说明仅计入已知部分。保留的正文仍限制为 1 MiB。
- 网络浮层展示真实目标主机、HTTP 协议（可用时）、HTTP/HTTPS、重定向及请求方式。
  源站通过云端 Proxy 得到的 Socket 初始化、IP、TLS 密码套件与证书信息，浏览器直连 Fetch
  不向页面提供；这些字段保留清晰的“不可用”，没有复制参考值。Cookie 页签同样说明浏览器
  不向页面提供 Set-Cookie，不用 document.cookie 冒充本次响应 Cookie。
- 指标支持悬停、键盘聚焦及点击/触摸打开；内容可悬停和滚动，Escape/外部点击关闭，
  打开时不移动焦点。使用现有 Reka 定位与浮层层级，窄窗口碰撞避让；页签单行横向滚动，
  不把指标塞入 tablist。小于 960px 的响应容器将契约区放到下方。

`outputs/apifox-workbench/response-acceptance.py` 使用独立规范和真实本机 HTTP/1.1 服务，
验证五个页签、指标浮层、gzip 编码/解码大小、受限跨域计时、204/404、1 MiB 截断、
真实前后置脚本、网络失败与成功重试，并检查三种窗口宽度及浅深色。脚本结束时删除
自己创建的规范。2026-11-15 验证通过 10 组响应专项检查、10 次真实请求，页面异常为 0；
详情与截图保存在 `outputs/apifox-workbench/response/result.json` 及同目录 PNG。

同日完整前端验证通过 74 个测试文件、423 项测试、生产构建、lint、格式检查及开发样式检查。
构建仍提示已有的大体积 chunk；这次响应面板变更没有调整应用的代码分包策略。

## 请求头、脚本与环境

- 文档页和调试页都不显示“请求代码”和“代码片段”入口；代码生成组件与纯函数保留供
  后续按需接入，不参与当前操作面板布局。
- Headers 中规范字段始终显示。标题旁的眼睛控制隐藏请求头，并在同一参数表中展示：正文类型和认证配置来自当前请求。
  浏览器模式说明 Host、Cookie、Content-Length 等受浏览器管理的字段；Agent 模式显示其
  User-Agent 及显式 Cookie，不声称使用浏览器 Cookie jar。完整值以“实际请求”为准。
- 浏览器模式的 Cookie 编辑通过同源 Cookie jar 发送，跨域手工 Cookie 会明确阻止。
  Agent 模式将显式 Cookie 头通过独立的请求头列表传给执行器，避免 Chromium Request guard
  静默剥除；不会把 SpecX 登录会话或浏览器 Cookie jar 转发给目标。Agent 不受目标 CORS 限制。
- 脚本引擎由 `openapiScriptSandbox`（Worker 生命周期/RPC）、`openapiScriptRuntime`
  （真实 QuickJS 执行与资源限制）、`openapiScriptApi`（兼容 API）、`openapiScriptHttp`
  （浏览器/Agent 子请求）组成。前后置步骤各自使用独立 VM，显式传递请求、作用域、
  临时变量、日志、断言和可视化；不在页面上下文执行用户代码。
- `specx.request.setHeader` 启用并更新同名请求头；兼容 `pm.request.headers` 的
  get/has/add/upsert/remove、method、URL/query、raw Body 和表单字段操作。
  只有前置修改会影响发送。后置的 `pm.request` 来自实际发送快照，包含最终请求头和正文；
  后置尝试修改请求时，控制台按当前界面语言提示请求已发送，每个步骤只提示一次。
  **响应 Headers 是服务器返回的响应头**；前置添加的 `X-Example` 在“实际请求”里检查，
  只有服务器主动回显时它才会出现在响应 Headers。
- 支持 `console.log/info/warn/error/debug`、`specx.console`、`specx.log`、
  `pm.test`（同步/Promise/done）、完整 Chai `pm.expect`、常用 `pm.response.to.have`
  和状态断言、JSON Schema 校验、response text/json/headers/setBody、Cookie 读取及运行信息。
  断言逐项显示在控制台；断言失败不会阻止后续测试或抹掉 HTTP 响应。
- 支持 `pm.variables`、`pm.environment`、`pm.globals` 的 get/set/has/unset/clear/toObject/
  replaceIn，以及支持动态表达式的 replaceInAsync。临时变量仅用于本次发送（跨所有前后置步骤）；
  环境/全局修改现有字段的本地覆盖值，不修改服务器定义。脚本新建的变量仅保留于客户端
  会话；未选环境时环境作用域仅对本次执行有效。unset 删除本地覆盖，后续请求仍可回退远程值。
  变量名含 token/secret/password 等敏感标识，以及被显式标记为敏感的环境字段不进入沙箱。
- `pm.sendRequest` 支持 Promise 和 callback、GET URL、显式 method/header/raw/urlencoded Body。
  子请求通过当前所选浏览器或云端 Agent 执行；浏览器受 CORS 限制，Agent 复用工作区/服务
  授权、地址限制与请求策略。不继承 SpecX 会话、页面 Cookie 或主请求认证。
  Promise、回调和一次性定时器完成后才提交脚本结果；取消/超时会终止 Worker 和子请求。
- 单步脚本最大 32 KiB、执行预算 5 秒、VM 内存 32 MiB；引擎/WASM 启动单独计时（最多 20 秒），
  避免将冷加载误报为脚本死循环。日志与断言各最多 500 项。前置失败指出步骤并阻止发送；
  后置失败保留响应与错误前的日志/断言，编辑后或重试清除旧错误。
- 兼容范围以这些可执行测试为准，不宣称整个 Postman/Apifox SDK 都已实现。
  暂不支持自动化集合编排、模块/团队变量、外部 npm/文件/数据库调用、脚本子请求文件上传、
  Nunjucks 控制流模板和可视化模板中的 JavaScript/远程图表库。帮助入口指向 Apifox 的公开
  [pm 脚本 API](https://docs.apifox.com/postman-script-api)；上述差异由 SpecX 的本节说明。
- 环境管理在 OpenAPI 规格和 OpenAPI 概览中可用。环境列表支持新建、复制、导出和删除，
  右侧提供前置 URL、环境变量以及保存操作。编辑和保存不会切换调试正在使用的环境。
- 每个环境有一条默认地址，也可以添加命名的服务地址，并在对应行设置请求代理。
  调试栏的前置 URL 按钮可以选择环境和服务地址，旁边的路径输入支持继续编辑。
  相对路径拼接到所选地址，完整 HTTP(S) URL 优先。环境选择按 SpecX 服务保存。已有环境时自动选择有效默认环境或第一个环境，空前置 URL 保持为空。
- 变量表只编辑一列值。新输入的普通值保存在本机，敏感值保留于当前会话。
  旧配置的服务端值仍可作为回退。导出和复制会排除敏感值。

`outputs/apifox-workbench/functional-acceptance.py` 使用本机 HTTP 回显服务验证 Params、
正文各模式、请求头、认证、真实 QuickJS 脚本、超时、重定向和响应校验；另行检查浏览器
Cookie 与环境增删改、优先级、刷新恢复，以及环境对话框在浅色/深色、不同宽度下的表现。
验证结果见同目录 `functional-result.json`。

## 内置云端 Agent

浏览器模式直接使用 Fetch；“自动”默认使用 SpecX 服务器内置 Agent。环境的前置 URL
高级设置可指定 Browser/Agent，默认继承底栏选择。公共文档试运行继续使用浏览器模式。

执行层 `crates/specx-request-agent` 只接收 `AgentRequest` 并返回 `AgentResponse`，无数据库、
会话或 UI 依赖。`crates/specx-api/src/request_agent.rs` 负责授权、DTO、并发与取消：

- `POST /api/specx/services/{service_id}/openapi/agent/requests/{request_id}` 提交请求。
- `DELETE` 同一路径幂等取消当前 actor 的请求，隔离租户、工作区、服务和请求 ID。
- body 使用 base64 保留二进制；headers 使用列表保留重复响应头。浏览器 Request 仅用于
  生成 FormData 边界及正文 bytes，显式 Cookie 独立保留。实际请求显示执行器生成后的头。
- 原生 HTTP 采集 DNS/TCP/TLS/TTFB/下载时间、IP/端口、协议及 Header/Body 大小，支持压缩、
  超时、取消、重定向和最多 1 MiB 响应保留。请求正文上限 8 MiB，API envelope 12 MiB，
  最多 16 个并发、10 次重定向、300 秒超时；请求大小的 Header 不重复计入上传正文。
- 默认仅允许公共 HTTP(S) 目标；管理者可设置逗号分隔的 `SPECX_AGENT_ALLOWED_ORIGINS`，
  精确允许内网 origin（含协议与端口）。每次 DNS 及重定向重新验证并固定地址，拒绝 URL
  凭据、CONNECT/TRACE、CRLF/Host/传输分帧头；TLS 证书正常校验，不继承服务器代理变量。
- 跨 origin 重定向清除 Authorization/Cookie，按 HTTP 规则重写 301/302/303 POST；
  不实现跨请求 Cookie jar。时间为重定向链总和，大小/地址为最后一跳；服务器连接总时间
  不包含浏览器到 SpecX 的网络耗时；Socket 初始化当前不单独提供。
- Unix 上的 OpenSSL/GnuTLS 从本次活跃连接读取 TLS 版本、密码套件、叶证书 CN、颁发者 CN
  和 GMT 到期时间。读取发生在响应头回调内，避免服务端关闭连接后句柄失效；不另发探测请求，
  不修改证书校验。普通 HTTP 显示“不适用”，证书没有 CN 或原生后端未开放时显示“未提供”。
  浏览器 Fetch 的 TLS 详情继续标记“不可用”。
- 实际请求使用 Agent 返回的 `requestMethod`、`requestHeaders`、`requestBodyBase64` 与最终 URL，
  保留变量/脚本处理后的真实 JSON、表单和二进制（十六进制）正文及字节数；303/301/302
  改写为 GET 时展示改写后的空正文；HEAD 展示传输层实际上传的零字节，不显示未发送的编辑草稿。
  GET 本身没有发送正文时明确提示，不用响应正文冒充请求正文。
- Header 表格随响应主面板滚动，表格和响应分栏外壳不再各自建立滚动区域。
- Agent 的 DNS 解析、连接、响应中断/协议、解压、响应头超限分别返回独立错误码与双语提示；
  失败日志仅记录类型，不输出目标 URL、请求头、凭据或正文。不自动重放失败请求。
- 目标的凭据、正文、Cookie 和完整 URL 不写日志或数据库；API 回包为 `no-store`。

环境 requestMode 使用迁移 24，在 SQLite/PostgreSQL 中同步存储。

## 2026-11-16 代理专项验收

`outputs/apifox-workbench/agent-acceptance.py` 使用隔离规范和本机真实 HTTP 服务，
通过 14 组检查、18 次实际请求：Path 提取与编码、行内/批量编辑、显式 Cookie 与重复
Set-Cookie、计时/网络/大小浮层、14 编码与语法切换、HTML/可视化沙箱、gzip、取消/重试、
浏览器 Fetch、multipart/二进制文件上传、重定向开关、环境代理与本地值优先/重置、
浅深色和 1512/844/390px 溢出检查。测试结束清理自己创建的规范及环境，结果与截图
见 `outputs/apifox-workbench/agent/`。执行器另有原生 HTTP 测试与 API 授权/取消隔离测试。

完整前端验证通过 81 个文件、443 项测试；API 48 项单元测试和 1 项契约测试、
原生 Agent 9 项测试通过。空 POST 的 Content-Length、上传正文与 Header 分开计数、
Path 数组/matrix 序列化均有回归用例。最后的提示调整另通过 i18n 一致性检查与浏览器
两种请求模式的自动 Headers 展示检查。生产构建、lint、格式、开发样式、Clippy、
SQLite/PostgreSQL 迁移一致性、Tauri locked 检查和 `git diff --check` 均通过。
Docker 镜像 `specx:apifox-agent-review` 构建通过；使用临时容器和独立数据库验证了
Web 登录与目录 API，随后清理容器。运行镜像包含 Agent 所需的 libcurl4。

截图对比确认响应分栏边界约 y562、标题行约 40px、正文工具栏单行约 40px；
参数表没有额外的“添加参数”按钮或强制横向滚条。SpecX 应用壳和上述能力差异仍保留，
不能将本次交付称为 Apifox 全站无差异复刻。

## 2026-11-16 参数与响应面板细化

参数表使用固定比例列和可收缩输入，新增行的操作列不撑宽表格。类型使用共用选择器，
Params 提供 string、integer、boolean、number、array、object 和引用类型；Path 限定四种标量类型，名称只读，值、类型及说明可编辑。
底栏只保留请求代理选择，不增加“调试”标题。

响应 Body 默认自动换行，行号可选择整行，右键菜单提供选区、当前行、JSON 路径/值复制及选择操作。
校验成功仅显示绿色结果标记，失败才展开错误详情；宽面板支持拖动分隔条、左右方向键、
Home/End 和双击复位，窄面板将详情放在正文下方。实际请求分为请求 URL、Header 表格和
带语法高亮的 Body，不再提供生成请求代码入口。

本轮从参考站读取正文的计算样式：12px 字号、16px 行距，浅色 JSON 键名 `#a31515`、
字符串 `#0451a5`、数字 `#098658`；深色继续使用独立配色。浏览器验证长声明参数、
自定义参数及响应内容在 1512/844/390px 下不撑宽容器，校验成功无侧栏、失败拖动、
行选择/复制、八个请求页签及隐藏请求头均可操作。验收数据保存于
`outputs/apifox-workbench/refinement-verification.json`，临时规范已清理。
## 2026-11-17 请求编辑器精简

已移除暂存、保存为用例、集合运行器及其本地存储、共享 API 和数据库访问接口。历史请求和会话内撤销/重做继续可用；历史迁移不回写、不删除已有数据。

Body 仅使用顶部导航选择格式，内容工具栏提供示例、JSON 格式化或表单批量编辑。
form-data 与 URL 编码表单共用参数类型选择，支持 primitive、array、object 及文档中的引用模型；multipart 还支持 file。引用模型会解析为字段输入结构。Path 的可选类型限定为 string、integer、boolean、number。批量编辑匹配同名字段并保留已选上传文件。

Headers 标题旁的眼睛显示/收起隐藏请求头，隐藏行与普通行共用表格。Agent 模式的勾选会传入 `disabledHeaders`，在认证、正文序列化和 cURL 默认头生成后抑制对应头。浏览器控制的 Host、Content-Length 等显示管理提示，不能假装由页面关闭。
取消生成的 Authorization 或 Cookie 后，不再要求不会发送的凭据；URL 中的 API Key 仍正常校验。

可视化采用 `pm.visualizer.set(template, data)`：通过 Handlebars 将响应 JSON 渲染为 HTML。后置操作及响应可视化空态均有“添加可视化示例”，添加后重新发送即可查看字段卡片。模板支持条件、循环、样式及转义值；在无脚本、无外部网络的 iframe 中展示，不兼容依赖模板 JavaScript 或远程图表库的用例。

浏览器使用 `https://jsonplaceholder.typicode.com/posts/1` 验证了 GET 可视化和 PUT multipart
上传：批量编辑保留文件，引用模型解析为字段，Params 引用按 OpenAPI 序列化；取消隐藏头后
实际请求中对应头确实消失。另验证取消 Authorization 后无凭据也能发送，重新勾选后仍要求凭据。
1512、844、390px 下文档及参数表均无横向溢出；窄单元格的引用键和值自动上下排列。
脚本、检查数据及截图位于 `outputs/apifox-workbench/controls-acceptance.py`、
`controls-verification.json`、`controls-*.png`。

完整前端验证通过 84 个文件、440 项测试；补充认证头回归后定向 29 项通过。
Rust workspace、API 契约、Agent 12 项原生传输测试、Tauri 4 项测试、Clippy、生产构建、
lint、格式、开发样式、迁移一致性及 `git diff --check` 通过。未配置 PostgreSQL 测试 URL，
实库分支没有执行。构建仍有既存的大体积 chunk 提示。


## 脚本引擎验证（2026-11-18）

`openapiScriptRuntime.spec.ts` 使用真实 QuickJS WASM，而非模拟执行结果；覆盖原始
X-Example/Handlebars 示例、请求/变量/表单/断言/异步子请求/超时/隔离/失败输出。
`openapiScriptHttp.spec.ts` 用真实本机 HTTP 接收端验证字节和请求头，另外检查 Agent 调用契约。
浏览器证据保存在 `outputs/script-engine/result.json` 与截图：原始模板、实际请求头、
前后置日志、失败恢复、多步骤变量传递、浏览器回显端和云端 Agent 的 JSONPlaceholder 请求。
生产构建下 10 组浏览器验收通过，浏览器运行异常为零；1512/844/390px 页面无横向溢出。
最终前端回归通过 87 个文件、473 项测试；类型检查、生产构建、lint、格式检查与
`git diff --check` 均通过。构建仍有既存的大体积 chunk 提示。

## Agent 错误诊断验证（2026-11-18）

原有“无法连接目标或解析响应”将不同失败合并为一个 502。现在保留连接错误身份，
另区分 DNS、响应读取/协议中断、解压和响应头超限，并在服务端仅记录类型。
`outputs/agent-errors/verify.py` 在真实 HTTP 服务制造传输中断、损坏 gzip 和超大响应头，
验证独立错误码及中文提示、失败不重放、成功重试清除旧错误；重启后 Agent 请求
JSONPlaceholder 成功。五组浏览器检查通过，无页面运行异常；原生执行器 14 项、
TLS 1 项、API 50 项、Web Host 5 项及前端错误/双语契约 30 项检查通过。
原始三次失败的旧日志只保留 502，没有请求地址或具体原因，不能据此判定根因。

## 动态变量与结构化参数（2026-11-18）

参考 [Apifox 动态值](https://docs.apifox.com/dynamic-values.md)、
[表达式语法](https://docs.apifox.com/dynamic-value-expressions.md)，并在项目 860863
实际操作参数类型、结构设置、引用模型、字段展开、动态值补全和生成器。参考截图和
本地验收产物位于 `outputs/dynamic-parameters/`（原始参考取证在本机 `/tmp/specx-ref-*`）。

- `openapiDynamicVariables` 使用与参考文档一致的 Faker 9.0.0-rc.1。支持
  `{{$string.uuid}}`、`{{$timestamp}}`、`{{$number.int(min=1,max=10)}}`、
  `{{$person.fullName(locale='en')}}` 等目录方法；包含参数、数组辅助函数、
  Apifox 字段投影、中文/英文数据、日期 format/add 管道，以及 MD5/SHA/Base64/大小写处理。
  普通变量可嵌套引用动态值；循环、未知方法和过量生成明确报错。参数只解析字面量，
  不执行表达式里的 JavaScript。528 条公开文档表达式有真实求值回归。
- 输入 `{{$` 或 `{{%` 触发补全，可继续输入方法名筛选；用方向键和 Enter 或鼠标选择，
  Escape 关闭。参数值（含 JSON/变量模式）、URL、正文、生成器表达式和脚本字符串共用目录。
  补全只替换当前变量的方法名，保留已有参数和转换管道；无参数时补齐 `}}`。
  点击参数值右侧的魔棒打开数据生成器，按类别浏览或搜索函数。选择函数后可编辑参数、
  添加处理函数、刷新预览，再将表达式插入光标位置或替换选中的文本。生成器沿用默认
  数据语言。预览不改变请求草稿；每次发送重新生成，实际值见“实际请求”。
  宽窗口中，生成器从输入栏右侧打开，并在可用空间不足时调整弹出方向。窄屏或矮窗口
  使用居中对话框，参数区可滚动，标题、预览和插入按钮可见。移动浏览器报告软键盘占用
  的空间时，对话框随可视区域调整。
- URL、Path、Query、Headers、Cookies、Auth、JSON 和表单正文共用发送阶段的解析器；
  JSON 模板区分字符串位置与数值/布尔位置，并处理表达式参数内的引号转义。
  `pm.variables.replaceInAsync` 及 `replaceIn` 使用相同引擎，支持动态值与嵌套变量。
  自增环境变量以本地覆盖/会话变量保存；未选环境时仅在本次执行中有效，不修改服务端定义。
- object、array 和引用类型旁提供结构及 style/explode 设置；引用入口可搜索模型、
  预览并确认，选中后立即展示字段。嵌套对象/数组递归编辑，已定义属性名称只读；
  JSON/变量模式保留原文并提示错误，取消结构编辑不改变草稿。结构修改仅属于当前
  调试参数，不写回共享模型。Path 仍限定四种标量类型。
- `openapiParameterValues` 在表达式替换后按引用模型还原数值和布尔类型；form-data
  与 URL 编码的对象字段作为 JSON 字符串发送。Query 的嵌套对象使用 deepObject
  bracket 路径，支持 URL 回填；这是对 OpenAPI 未规定的嵌套 deepObject 的明确扩展。
  form/simple 的平面对象继续遵循原有规范，遇到不支持的嵌套格式会阻止发送。
- 本轮没有实现 Nunjucks 的 for/if 控制流和远程数据提取动态源；数据语言支持中文、
  英文。可视化脚本的隔离边界保持原有行为，不将用户表达式交给页面 eval。

最终前端回归通过 90 个测试文件、1013 项测试；其中动态表达式包含 528 条公开文档语法用例。
生产页面完成 14 组实际交互验证（浏览器与云端 Agent、8 次 HTTP 请求），无浏览器异常；
1512/844/390px 参数表无横向溢出，390px 浅深色生成器浮层均在视口内。类型检查、生产构建、lint、格式、开发样式及
`git diff --check` 均通过。详细证据见 `outputs/dynamic-parameters/result.json`。
