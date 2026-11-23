# 第三方声明

本项目包含或改编了下列第三方材料。各项材料继续遵循其原许可；本项目品牌、
产品命名、更新密钥和发布渠道由 SpecX 独立维护。

## Apache-2.0 通用界面代码

若干通用界面控件、主题/布局工具和交互逻辑改编自
[t8y2/dbx](https://github.com/t8y2/dbx)，参考提交为
`d53516981e814c479783db0194f50f456e44668b`。相关代码位于
`apps/desktop/src/components/ui/`、`composables/`、`lib/` 和 `styles/`，并已按
SpecX 的 API、资源模型和品牌改造。改编内容仍依据 Apache License 2.0 分发；
完整文本见 [`LICENSES/THIRD-PARTY-APACHE-2.0.txt`](LICENSES/THIRD-PARTY-APACHE-2.0.txt)。

## Geist 字体

Geist 字体随其上游许可分发；许可文件见
[`apps/desktop/public/fonts/Geist-LICENSE.txt`](apps/desktop/public/fonts/Geist-LICENSE.txt)。

## jq 响应筛选

浏览器工作台按需加载 [jq-wasm](https://github.com/owenthereal/jq-wasm) 提供的 jq。
分发产物包含 jq-wasm、jq、Oniguruma 与 Emscripten 的许可声明，见
[`apps/desktop/public/licenses/JQ-WASM.txt`](apps/desktop/public/licenses/JQ-WASM.txt)。

## 模型代码生成

浏览器按需加载 [quicktype-core](https://github.com/glideapps/quicktype) 26.0.0，
用于将 OpenAPI 模型转换为语言类型与转换器，依照 Apache-2.0 分发。完整许可随
产物提供，见 [`apps/desktop/public/licenses/QUICKTYPE.txt`](apps/desktop/public/licenses/QUICKTYPE.txt)。

## 请求代码语法高亮

请求示例使用 [highlight.js](https://highlightjs.org/) 11.x 的语言语法，依照 BSD-3-Clause 分发。
完整许可随产物提供，见 [`apps/desktop/public/licenses/HIGHLIGHT-JS.txt`](apps/desktop/public/licenses/HIGHLIGHT-JS.txt)。

## 响应编码与可视化

响应查看使用 [iconv-lite](https://github.com/ashtuchkin/iconv-lite) 与
[buffer](https://github.com/feross/buffer) 解码原始字节；后置脚本的可视化模板
使用 [Handlebars](https://handlebarsjs.com/)。三者依照 MIT 许可分发，完整声明见
[`apps/desktop/public/licenses/RESPONSE-VIEWER.txt`](apps/desktop/public/licenses/RESPONSE-VIEWER.txt)。

响应编辑器使用 CodeMirror 的 HTML/XML 语言包，依照 MIT 许可分发。

本声明仅记录来源和适用许可，不授予任何第三方商标或品牌的使用权。

## 脚本执行与断言

隔离脚本引擎使用 [QuickJS](https://bellard.org/quickjs/) 和
[quickjs-emscripten](https://github.com/justjake/quickjs-emscripten) 0.31.0，
断言在该引擎内运行 [Chai](https://www.chaijs.com/) 5.3.3；均依照 MIT 许可分发。
完整声明见 [`apps/desktop/public/licenses/SCRIPT-ENGINE.txt`](apps/desktop/public/licenses/SCRIPT-ENGINE.txt)。

## 动态值生成

动态表达式使用 [Faker](https://fakerjs.dev/) 9.0.0-rc.1、[date-fns](https://date-fns.org/) 4.1.0、
[JSON5](https://json5.org/) 2.2.3 与 [@noble/hashes](https://github.com/paulmillr/noble-hashes) 1.8.0，
均依照 MIT 许可分发。完整声明见
[`apps/desktop/public/licenses/DYNAMIC-VALUES.txt`](apps/desktop/public/licenses/DYNAMIC-VALUES.txt)。
Apifox 公开文档中的短表达式作为兼容语法测试夹具，不包含其应用实现或品牌资产。
