# UI 基础层

Web 与 Tauri 共用 `apps/desktop`。基础层提供界面控件、主题、语言、键盘操作和
窗口内状态；领域资源通过 SpecX store、类型和 API client 接入。第三方来源和
许可记录集中在仓库根目录 `THIRD_PARTY_NOTICES.md`。

## 模块职责

| 模块 | 职责 | 约束 |
| --- | --- | --- |
| `components/ui/` | Button、Input、Select、菜单、提示和对话框等控件 | 不读取业务 store 或直接调用 API |
| `components/layout/` | 顶栏、侧栏和标签栏壳层 | 只转发用户动作并组合视图 |
| `components/sidebar/` | 分组/服务树、筛选、定位、拖放和内联命名 | 领域操作交给 navigation store |
| `components/settings/` | 分类设置、外观和快捷键 | 用户可见文本必须进入双语资源 |
| `composables/` | 主题、拖放、面板尺寸、滚动和快捷键生命周期 | DOM 监听需在卸载时清理 |
| `lib/app/`、`lib/common/`、`lib/editor/` | 偏好、几何工具、快捷键与展示格式 | 纯逻辑优先，避免依赖组件实例 |
| `lib/sidebar/`、`lib/specifications/` | 目录视图和规范类型适配 | 不持有持久化真相 |
| `stores/` | 页面状态与领域命令 | 通过 backend client 调用服务端事实 |
| `lib/backend/` | HTTP/Tauri transport、错误解析和请求类型 | 两种 Host 遵守同一 API 契约 |
| `i18n/`、`styles/` | 中英文资源、tokens、主题和布局样式 | 不硬编码用户可见文本或重复主题状态 |

依赖方向为组件 → store → client/纯函数 → transport。前端不访问数据库。服务、
分组、标签、规范和修订由 API 管理；主题、字体、语言、快捷键、筛选和面板尺寸
属于当前设备偏好。

## 通用交互

- 普通文字提示统一使用 `AppTooltip.vue`，延迟 300ms、默认位于触发元素下方，
  浮层不接收指针事件。带操作的内容使用菜单或对话框。
- 顶栏主题按钮只在浅色与深色之间直接切换；跟随系统属于设置中的持久化选项，
  选择后仍响应系统外观变化，不在顶栏重复提供下拉选择。
- `LightTooltip.vue` 负责普通提示的固定方向、300ms 延迟、关闭延迟、视口边界调整
  和 pointerdown/scroll/resize 关闭；提示浮层不接收指针事件，不能遮挡触发按钮。
- Tauri 窗口在 Windows/Linux 运行时关闭原生装饰并使用应用内控制，在 macOS 使用
  Overlay traffic light；平台差异集中在 `useWindowControls` 与 Tauri Host，不散落在业务组件。
- 快捷键从 `shortcutRegistry.ts` 单一注册表派生运行逻辑、提示和设置界面。每项
  动作需声明分类、翻译 key 和默认组合；修改、停用、重置和冲突行为需保持一致。
- 分组与服务创建通过队列显示待保存节点。确认后合并服务端响应和 ETag；失败
  保留草稿，重试复用原 ID、请求内容和幂等键，不刷新整个目录。
- 主题、字体和语言使用 SpecX 自己的本地存储键。不要复用其他应用的存储命名、
  API endpoint、品牌素材或发布身份。

## 扩展规则

新增通用交互时先确定状态归属和键盘/指针语义，再决定落在控件、composable、
纯函数或 store。多个页面共享且不依赖领域对象的行为才进入基础层；服务和规范
特有逻辑留在对应视图、store 与 API。修改用户可见行为时补充两种语言、稳定的
测试和窄窗口验证。新增 API 错误使用稳定错误码并遵守
[错误契约](api-error-contract.md)；新增界面文案遵守[国际化规范](i18n.md)。

当前 UI 支持简体中文和英文，默认简体中文。布局与主题偏好在本机生效；分组、
服务等资源事实通过服务端共享。桌面更新属于 Host 能力，详见
[版本与发布规范](../guides/desktop-releases.md)。
