# specX 共用前端

这里是 Web 与 Tauri 共用的 Vue 3 + Pinia 应用。Web 部署也使用这里的
构建产物。Rust 的 Web Host 位于
`crates/specx-web`，原生 Host 位于根目录 `src-tauri`。

| 路径 | 职责 |
| --- | --- |
| `src/App.vue` | 组装应用壳、登录状态、全局对话框与提示 |
| `src/components/layout/` | 顶部栏、侧栏、标签栏；只转发界面动作 |
| `src/components/brand/` | JSX/TSX 编写的 SVG 品牌图标与完整字标 |
| `src/components/sidebar/` | 资源树的键盘、右键菜单、内联命名、拖放与定位 |
| `src/components/settings/` | 设置分类、外观、导航偏好与目录导入 |
| `src/components/ui/` | 通用控件，不依赖 specX 领域 |
| `src/stores/workspace.ts` | 工作区事实、打开的服务/规范标签与加载生命周期 |
| `src/stores/navigationStore.ts` | 目录命令、命名草稿、筛选、定位与 API 接线 |
| `src/stores/settingsStore.ts` | 当前设备的导航偏好、经典/分隔布局和设置页状态 |
| `src/lib/sidebar/` | 目录布局算法和 SpecX catalog 适配，便于独立测试 |
| `src/lib/backend/` | 统一 client、HTTP/Tauri transport、错误与安全存储 |
| `src/lib/transfer/` | 目录 JSON 协议、校验、导入与导出 |
| `src/composables/` | 主题、拖放、尺寸、标签滚动和快捷键等界面能力 |
| `src/i18n/` | 简体中文和英文资源、语言偏好与运行时翻译 |
| `src/styles/` | RGB 基础 tokens、主题色板、OKLCH 渐进增强、应用壳和资源内容样式 |
| `src/types/` | API、领域视图与导航契约 |

依赖方向为组件 → store → client/纯函数 → transport。组件不得直接访问
数据库或调用原生业务命令。`specx-core` 的领域规则也不迁到前端。

```sh
pnpm --dir apps/desktop dev
pnpm --dir apps/desktop check:dev-styles
pnpm --dir apps/desktop format:check
pnpm --dir apps/desktop test
pnpm --dir apps/desktop build
```

浏览器开发另需运行 `SPECX_PASSWORD=... pnpm dev:backend`；前端代理默认指向
`http://127.0.0.1:4224`。桌面开发在仓库根运行 `pnpm dev:desktop`。

桌面版本、自动/手动更新检查、签名包安装和双语更新日志见
[桌面发布规范](../../docs/guides/desktop-releases.md)。更新属于 Host 与应用壳能力，
不进入 Core 或业务 API；版本号由构建注入，不能写死在语言资源中。

默认 Web 开发端口为 5173，桌面为 1420；端口占用时直接报错，不自动跳到
其他端口。显式改端口可使用 `pnpm --dir apps/desktop dev --port 5174`。
迁移框架或新增 Vite 插件后，应重启运行中的开发服务，源码热更新不能让旧
进程加载新增插件。`check:dev-styles` 检查实际开发 CSS 转换；也可追加 URL
检查已运行的实例，例如
`pnpm --dir apps/desktop check:dev-styles http://127.0.0.1:5174`。该检查应同时
确认没有原样输出的 Tailwind 指令，以及布局工具类和字体 token 已生成。

支持 Vue `.vue`、`.jsx` 和 `.tsx` 组件混用。JSX 使用 Vue 语义，由
`@vitejs/plugin-vue-jsx` 转换；TypeScript 使用 `jsx: preserve` 和
`jsxImportSource: vue`，无需 React。Tailwind 扫描 JSX/TSX，Vitest 也识别
这些扩展名的 `.spec` 文件。推荐使用 `defineComponent` 配合 `setup` 返回
渲染函数，使 props 保持响应式并支持开发时热更新。

界面支持 `zh-CN` 和 `en`，默认使用简体中文，语言偏好保存于本机的
`specx-locale`。新增或修改用户可见的界面文案时，必须同时更新
`src/i18n/locales/zh-CN.ts` 与 `src/i18n/locales/en.ts`，并在组件中通过
`translate("namespace.key")` 读取。翻译 key 会按资源路径进行类型检查，英文资源以
中文资源的结构为约束，类型检查会发现无效 key 或遗漏的文案。动态文案使用命名参数，例如
`translate("sidebar.tag", { name })`；不要拼接已翻译的短语。日期应使用
`formatDate` 并传入当前 locale。服务、分组、标签名称、规范标题和规范正文均是
工作区数据，不属于界面翻译资源；内部伪分组名称应通过 `displayGroupName` 显示。
API 新错误优先使用稳定的错误码，并在两种语言资源中补充对应文案；不依赖服务端
临时变化的错误详情作为 UI 文案。

完整的 key、插值参数与 locale 扩展约定见
[国际化规范](../../docs/architecture/i18n.md)。当前 v1 错误响应契约见
[API 错误规范](../../docs/architecture/api-error-contract.md)，跨层代码约定见
[代码规范](../../docs/architecture/code-conventions.md)。

普通文字提示统一使用 `components/ui/AppTooltip.vue`：采用 Reka Tooltip，
默认下方展示、延迟 300ms，内容与定位容器均不接收指针事件。
不要为某个按钮单独实现提示定位、使用零延迟或把操作按钮放进文字提示中。
包含实际操作的浮层应使用菜单或对话框。

快捷键动作统一定义在 `lib/editor/shortcutRegistry.ts`，运行时通过
`shortcutStore.matches` 匹配；界面提示通过 `shortcutStore.hint` 生成，不能
再硬编码 `Mod+B` 等组合。新增动作需同时声明分类、翻译 key 和默认组合，接入
对应处理函数，并覆盖修改、停用、重置后的真实触发行为。录入、IME 组合输入、
弹窗与按键重复不会触发全局动作；原生文本编辑及目录导航按键予以保留。
当前快捷键偏好保存在本机，分类包括全局与目录，重复组合不会保存。

分组/服务的行内创建由 `navigationStore` 提交给 `createQueueStore`。
队列立即显示待保存节点，服务器确认后将规范响应合并入 `workspace`；不要为了
创建一个节点重新加载整个目录，也不要用全目录快照回滚失败操作。创建失败保留
名称与错误信息，支持手动重试和移除本地草稿；重试必须复用原 ID、原请求内容
及幂等键。浏览器本地存储可用时，刷新后恢复未确认草稿并等待用户重试。
未保存的分组不能创建子项，服务详情和后续写入在服务器确认后启用。
工作区隔离、退出登录后的迟到响应、并发刷新与服务 ETag 必须保留回归覆盖。

品牌组件可在 `.vue` 模板或 JSX 中直接使用：

```tsx
import { defineComponent } from "vue";
import { SpecxLogo, SpecxMark } from "@/components/brand";

export default defineComponent({
  setup: () => () => (
    <div class="flex items-center gap-4">
      <SpecxLogo size={40} animated />
      <SpecxMark size={24} monochrome decorative />
    </div>
  ),
});
```

`size` 是高度（像素）；`SpecxMark` 为正方形，`SpecxLogo` 宽度为高度的
3.5 倍。`monochrome` 跟随 `currentColor`，`decorative` 隐藏重复的读屏
信息，`label` 默认为 `specX`。`animated` 仅播放一次入场动画，并遵循
`prefers-reduced-motion`。字标全部由 SVG 路径绘制，不依赖字体；每个
实例通过 Vue `useId()` 隔离渐变、遮罩和标题 ID，可在同页重复使用。

通用交互、主题、布局、快捷键和资源适配的模块职责见
[UI 基础层](../../docs/architecture/ui-foundation.md)。默认使用经典紧凑布局，
设置中也可切换现代分隔布局。样式变更除类型检查和构建外，应覆盖浅色/深色、
菜单、对话框与窄窗口，并检查正在运行的开发入口。
