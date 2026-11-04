# 贡献接口最小约定

一期只需要让 Host 安全地调用 Markdown 预览，并为未来 Provider 留出可演进边界。

## 请求

```json
{
  "contribution": "spec.preview",
  "specKind": "markdown",
  "workspaceId": "…",
  "serviceId": "…",
  "specificationId": "…",
  "contentRef": "…",
  "protocolVersion": 1
}
```

Core 在发送前固定授权上下文和内容版本；Provider 只能通过引用读取允许的正文。响应包含 `status`、`rendered` 或 `diagnostics`，以及实际使用的贡献版本。未知类型、无授权、内容过期和 Provider 不可用都返回可区分的诊断。

## 安全边界

贡献实现不直接访问数据库、浏览器 token、任意绝对路径或其他工作区资源。纯 UI 贡献可以在 Host 进程中运行，但必须通过受控组件和内容清洗；未来执行用户输入或高资源任务时再使用隔离进程。接口升级采用显式协议版本，不能以包 ID 或默认 Provider 猜测兼容性。
