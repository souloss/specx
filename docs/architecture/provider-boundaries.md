# Core、Host 与 Provider 边界

## Core

Core 是服务资产的事实源，负责租户和工作区上下文、服务目录、目录层级、标签、规范身份与内容引用、授权、审计和持久化。Core 提供稳定的查询和命令接口，不依赖具体前端框架或规范渲染库。

## Host

Host 负责应用壳：侧边栏、搜索筛选、顶部两侧工具栏、最小宽度约束、标签页、面板生命周期、主题和用户会话状态。Host 通过 Core API 读取服务事实，通过贡献接口挂载规范预览和未来专属面板。Host 不直接访问数据库或 Provider 私有 store。

## Provider/贡献实现

Provider 负责声明的格式或能力实现，例如 Markdown 预览。它接收 Host/Core 发出的最小授权上下文，返回版本化结果和诊断；不复制身份系统，不直接写核心表，不将服务凭据暴露给浏览器。

一期不要求独立 Provider 进程、复杂插件包或插件市场。纯 UI 的 Markdown 贡献可以随 Web 发布；未来涉及不可信代码、用户输入执行或高资源消耗时，再使用受管进程或执行池隔离。

## 请求方向

```text
Web Host ── Core API ── Core domain/storage
    │
    └──── contribution bridge ── Markdown renderer / future provider
```

核心事实流向是 Core → Host/贡献；贡献不能反向改变服务身份、目录权限或工作区边界。贡献的不可用状态要能被用户理解，并且不会阻塞服务目录和已有规范读取。
