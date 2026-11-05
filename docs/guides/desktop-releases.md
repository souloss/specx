# 桌面版本与发布规范

本文件与根目录 `release.config.json` 是桌面发布的维护入口。配置仓库名、更新
公钥、应用版本和双语日志后，GitHub Actions 按 Tag 构建安装包并发布 Tauri
更新清单。更新包由 Tauri 官方 updater 验签；业务 API 和 Core 不依赖发布服务。

## 版本与 Tag

- 使用稳定版 SemVer `X.Y.Z`，正式 Git Tag 为 `vX.Y.Z`。首期不发布预发布版，
  不覆写已发布的 Tag、安装包或更新日志。
- 根 `package.json` 是桌面应用版本的唯一操作入口；前端 package、Tauri 配置、
  桌面 Cargo manifest 与锁文件必须保持一致。Rust workspace crate 可独立演进。
- Tag 必须与应用版本和两种语言日志的首条记录一致。工作流拒绝格式错误、
  版本不一致、日志缺失或晚到的旧版本任务覆盖当前 latest。
- API 契约版本与桌面应用版本分别管理，不通过桌面 Tag 暗示 API 的兼容性。

## 更新日志

事实文件为 `releases/changelog.zh-CN.json` 和 `releases/changelog.en.json`。
每个版本包含 `tag`、`name`、`date`、`sections`；章节类型为 `added`、
`improved`、`fixed`、`changed` 或 `removed`，条目包含 `title` 与 `desc`。
两份文件必须有相同的版本顺序与日期，最新版本放在首项。

在两个 `unreleased.sections` 中同步记录面向用户的变化。内容要描述用户获得的
结果，不直接复制提交列表、调试细节或内部重构。发布准备会将未发布内容归档到
目标版本并清空 `unreleased`；已发布版本的日志不可修改。

新增版本条目使用以下结构，并将标题与说明分别翻译为中文和英文：

```json
{
  "type": "added",
  "title": "新增",
  "items": [
    { "title": "用户可见的能力", "desc": "它解决的问题或带来的结果。" }
  ]
}
```

应用内附带当前已知的双语历史，联网时按界面语言读取 GitHub Release 中对应的
JSON。远端读取失败时保留内置历史；更新清单与日志都来自公开 HTTPS 资源。

## 密钥与仓库配置

`release.config.json` 是可公开提交的分发配置，包含仓库 `owner/name` 和 Tauri
updater 公钥。fork 后改为自己的仓库，并为自己的应用重新生成独立密钥；更新
信任根不能与其他发行方共享。

本机更新私钥与密码保存在仓库外：

```text
~/.config/specx/specx-updater.key
~/.config/specx/specx-updater.password
```

私钥用于签署更新包，密码用于解锁私钥。两者不要提交、贴进 issue/聊天或输出到
CI 日志。私钥和密码应分别备份到受控的密码管理器或离线安全介质。丢失任一项后，
已发布客户端无法信任用新密钥签署的更新；密钥轮换需要先设计客户端信任迁移。

GitHub Actions 私密值应保存在目标仓库的 **Settings → Secrets and variables →
Actions → Repository secrets**：

| Secret | 内容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | 私钥文件的完整文本 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥密码文件的完整文本 |

可用 GitHub CLI 从本机文件直接写入 Secret，不会把内容放进命令参数或终端输出：

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo souloss/specx < ~/.config/specx/specx-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo souloss/specx < ~/.config/specx/specx-updater.password
```

公钥不是 Secret；它保存在 `release.config.json`，fork 时与仓库名一起修改。macOS
Developer ID 签名和公证是独立于更新包签名的可选 GitHub Secrets：
`APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、
`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`。面向普通 macOS 用户分发前应配置
并在真实系统验收安装与公证状态。

## 首次发布 `v0.0.1`

发布前更新两份 `unreleased` 日志，然后执行：

```sh
pnpm release:prepare 0.0.1
pnpm release:check v0.0.1
pnpm test:release
pnpm test:specx
```

检查生成的版本号和双语日志，提交后再给同一提交创建 Tag：

```sh
git add package.json apps/desktop/package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json releases
git commit -m '发布 v0.0.1'
git tag -a v0.0.1 -m '发布 v0.0.1'
git push origin main
git push origin v0.0.1
```

以后每次发布前先把应用版本递增到目标版本，再填写 `unreleased`，运行同一组
检查，并提交准备结果。补丁修复使用新的 patch 版本；已发布内容不可复用。

## GitHub Actions 行为

`.github/workflows/ci.yml` 在主分支 push 和 Pull Request 上运行格式检查、前端
类型与构建、Rust 检查和测试。`.github/workflows/release.yml` 只响应 `vX.Y.Z`
Tag：先校验版本、双语日志和签名配置，再创建草稿；Windows x64、Linux x64、
macOS arm64/x64 构建并上传安装包及 `latest.json`。全部平台的签名资产齐备后，
工作流上传双语日志并将草稿发布为 latest。失败会保留草稿，不应手工发布缺少平台
的版本；修复后重跑失败的工作流。工作流按 Release ID 读写构建资产，重跑同一
Tag 时复用最新草稿并删除旧的同名草稿，避免同名 Draft 导致安装包与
`latest.json` 串包。

当前首版未配置 Apple Developer ID 证书，macOS 构建使用 `--no-sign`，但更新包
仍使用 Tauri updater 私钥签名。配置 `APPLE_CERTIFICATE` 等 Secrets 后，应移除
macOS matrix 的 `tauri_args: --no-sign`，再发布新的 patch 版本完成系统签名与公证。

首次发布后，使用前一版本的真实安装包检查签名更新、下载进度、安装确认、重启、
版本变化与数据保留。CI 不能替代各操作系统上的实际安装和升级验收。
