# API 错误契约与国际化规范

本文定义当前唯一错误响应格式、稳定错误身份和客户端本地化规则。实现位于
`crates/specx-api/src/errors.rs`；API、Web 登录、HTTP 与 Tauri 使用同一契约。
其他 API 约定见[API 契约](api-contract.md)，语言资源维护见[国际化规范](i18n.md)。

## 分层职责

- `specx-core` 返回有类型的领域错误，不依赖 HTTP、Vue 或 locale。
- `specx-api` 将领域错误和 API 边界错误映射到唯一 catalog，并负责 HTTP 状态和 JSON 序列化。
- `specx-web` 的登录、会话和 Host 错误使用同一 catalog，不另建错误格式。
- 前端按 `messageKey` 与 `messageParams` 使用当前语言包渲染摘要；组件不按文案分类错误。

HTTP status 表示传输结果；`code` 表示稳定机器身份；`messageKey` 表示用户摘要的本地化入口。

## 响应格式

所有 API 错误使用以下 v1 envelope：

```json
{
  "version": 1,
  "code": "precondition_failed",
  "messageKey": "apiError.preconditionFailed",
  "messageParams": {}
}
```

- `version` 是错误 envelope 的结构版本，不是 API 或应用发布版本。客户端只接受已实现的版本；结构不兼容时升级此值。
- `code` 是稳定、唯一、供程序判断的错误身份，使用小写 `snake_case`。不同可区分的错误不得共用 code；HTTP status 不能代替 code。
- `messageKey` 是语言包中的稳定 key。用户文案可以调整，key 的语义不能改变。它不用于控制流。
- `messageParams` 是命名插值参数对象。每个参数名和类型都必须由 catalog 明确声明；当前错误均为空对象。新增参数时同步扩展 Rust 类型、OpenAPI schema 和前端校验。禁止传入密码、token、请求体、SQL、异常对象或任意嵌套数据。

响应不包含 `message`、`detail`、异常描述或调试信息。服务端不按 `Accept-Language` 翻译错误；每个客户端自行按当前 locale 渲染。内部诊断只写入经过脱敏的服务端日志。

## Code 规范

1. 每个可区分错误只定义一个 code，例如 `precondition_failed`、`duplicate_slug`。错误身份变细时新增 code，不通过复用泛化 code 隐藏差异。
2. code 使用描述原因或业务条件的英文小写 `snake_case`；不包含 HTTP 数字、语言、文案、实现类型名或临时内部细节。
3. 同一错误在 HTTP、Web 登录和 Tauri 中保持相同 code。status 根据协议语义单独选择，例如缺少 `If-Match` 是 `428`，版本不匹配是 `412`。
4. code 是 wire contract。发布后不得改变含义、复用或静默重命名；废弃时停止返回并保留记录。尚未发布的实现可在同一变更内调整。
5. `messageKey` 和 code 一一对应于 catalog 条目。新增或修改条目时，英文与简体中文必须同时提供 key，命名插值参数必须完全一致。
6. 按具体来源错误映射，不格式化 Rust `Debug`、数据库错误、框架 rejection body 或任意异常作为响应字段。

## 客户端规则

- `apps/desktop/src/lib/backend/types.ts` 是 HTTP/Tauri 共用解析边界。它要求 `version: 1`，校验 code、key、参数名与参数值，再调用 i18n。
- 客户端只支持当前 envelope；无版本旧对象和未知版本都视为无效响应，不读取 `message`，也不维护旧 code 映射。
- 当前 locale 缺少有效翻译时使用中文 fallback；响应无效或 key 不存在时显示通用本地化摘要。绝不把服务器原文或 key 直接显示给用户。
- 控制流按 HTTP status 和稳定 code 判断，不读取 `messageKey` 或翻译文案判断权限、重试或业务状态。

## 实现与验证

- `errors.rs` 是唯一后端 catalog；`ErrorKind` 覆盖领域错误和框架拒绝，`CoreError` 的所有变体必须显式映射。
- `crates/specx-api/tests/fixtures/error-catalog.json` 是跨语言契约快照。Rust 测试验证响应形状、code/key 唯一性和 catalog 一致性；前端测试验证所有 key 在中英文存在、占位符一致及安全 fallback。
- OpenAPI 注解和 `openapi-contract.json` 必须声明四个 envelope 字段。新增错误需覆盖来源映射、status、code、key、参数和失败路径。
- 改变契约时，同一变更更新 API、Web 登录、Tauri、HTTP 解析、OpenAPI、catalog fixture、两种语言及本文。不得另加旧格式适配层。

## 新增错误完成条件

1. 在 catalog 定义唯一 code、status、messageKey 和类型明确的参数，并从具体来源错误映射。
2. 更新 `en` 与 `zh-CN` 的相同 key 和插值参数。
3. 更新 catalog 与 OpenAPI fixtures，并增加 Rust 和前端测试。
4. 验证 HTTP 与 Tauri 返回相同身份，错误响应不泄漏敏感数据或内部调试文本。
