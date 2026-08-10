# 宜租网 AI 找房客户端

这是“企业登录 → 描述找房需求 → AI 结构化 → 确认补充 → 房源推荐 → 生成招商线索”的第一条可运行垂直切片。客户端使用 uni-app、Vue 3、TypeScript 和 Pinia，支持 H5 与微信小程序构建。

当前数据和身份均为本地演示：房源来自仓库内完全虚构的 fixture；模拟会话和线索只存在 Rust BFF 进程内存中，重启即丢失；不会发送短信，也不会通知真实招商顾问。

## 环境要求

- Node.js 18.18 或更高版本
- npm 9 或更高版本
- Rust stable 与 Cargo（启动本地 BFF 时）
- 微信开发者工具（预览微信小程序时）

## 安装和检查

```powershell
cd <repo-root>\apps\yizu-client
npm ci
npm run type-check
npm run test
```

## H5

先在仓库根目录按下文启动 Rust BFF，再新开终端：

```powershell
cd <repo-root>\apps\yizu-client
$env:VITE_YIZU_DEMO_MODE="true"
$env:VITE_YIZU_API_BASE_URL="http://127.0.0.1:8080"
npm run dev:h5
```

生产构建命令为 `npm run build:h5`，输出在 `dist/build/h5`。生产构建不会因为 `VITE_YIZU_DEMO_MODE` 自动获得模拟身份；客户端只在 `import.meta.env.DEV` 与显式演示开关同时成立时展示模拟登录入口。

## 微信小程序

```powershell
cd <repo-root>\apps\yizu-client
$env:VITE_YIZU_DEMO_MODE="true"
$env:VITE_YIZU_API_BASE_URL="http://127.0.0.1:8080"
npm run dev:mp-weixin
```

或执行 `npm run build:mp-weixin`，然后在微信开发者工具中导入 `apps/yizu-client/dist/build/mp-weixin`。仓库不包含真实微信 AppID；联调人员需在自己的开发者工具环境配置合法 AppID 和请求域名。当前 `manifest.json` 关闭了 uni 统计。

## 本地 Rust BFF

本切片复用现有 Dioxus Fullstack 进程，没有创建第二个后端。开发模拟登录还需要服务端显式开关：

```powershell
cd <repo-root>
$env:YIZU_MINIAPP_DEV_AUTH_ENABLED="true"
$env:YIZU_MINIAPP_AI_PROVIDER="local"
dx serve --features server
```

若本机 Dioxus CLI 的启动参数与仓库版本不同，以根目录 `Dioxus.toml` 和现有项目启动方式为准。默认 API 前缀为 `/api/miniapp/v1`。

## 隔离 HTTP E2E

不启动完整根应用即可验证 uni-app JSON 契约与 Rust BFF 的 5 条真实 HTTP 路径：

```powershell
cd <repo-root>
cargo test --features server miniapp_http_e2e
```

测试只绑定 `127.0.0.1` 随机端口，固定使用本地解析器、虚构 fixture、内存会话和内存线索；不会读取百炼 Key，不会初始化 SpacetimeDB、对象存储、短信、设备或其他园区模块，结束后自动关闭并验证端口释放。

## 表单契约

- 镇街以 `/metadata/options` 为权威来源；首页与确认页共用 metadata Store，请求失败时使用完整 33 镇街降级列表并保留用户选择。
- 用户界面只显示元/月或元/平方米/月；API、Pinia 和 Rust 内部使用整数分。金额输入支持整数或最多两位小数，拒绝负数、过高金额和多余小数位。
- 支持字段用 `constraint_priorities` 标记 `hard` 或 `preference`。未满足或无法验证的硬条件都会阻断客户端提交，Rust 服务端还会重算并最终拒绝。
- 缓存是 v2 versioned envelope；损坏或旧版缓存会安全清理。页面统一按认证 → 需求 → 表单同步 → 守卫的顺序恢复。

## AI Provider

- `YIZU_MINIAPP_AI_PROVIDER=local`：默认，完全本地、确定性解析，不需要网络或密钥。
- `YIZU_MINIAPP_AI_PROVIDER=bailian`：由 Rust 服务端调用百炼；密钥只从 `ALIYUN_BAILIAN_KEY` 读取，模型名从 `YIZU_MINIAPP_AI_MODEL` 读取。
- `YIZU_MINIAPP_BAILIAN_ALLOW_LOCAL_FALLBACK=true`：仅 debug 构建允许显式回退 local，并在响应中提供 `fallback_reason`；release 构建不允许静默降级。

客户端环境变量：

- `VITE_YIZU_API_BASE_URL`：BFF 基础地址；开发环境未配置时使用 `http://127.0.0.1:8080`，生产语义下缺失会报错。
- `VITE_YIZU_DEMO_MODE`：仅本地开发设为 `true`；不是认证凭据。

服务端环境变量：

- `YIZU_MINIAPP_DEV_AUTH_ENABLED`
- `YIZU_MINIAPP_AI_PROVIDER`
- `YIZU_MINIAPP_AI_MODEL`
- `YIZU_MINIAPP_BAILIAN_ALLOW_LOCAL_FALLBACK`
- `ALIYUN_BAILIAN_KEY`

不要创建或提交真实 `.env`，不要把百炼 Key 放入任何 `VITE_` 变量。

## 演示边界

- 18 套房源全部虚构，无真实租户、地址、联系人、图片或生产导出数据。
- AI 推荐只使用可出租且满足资格的自营或 L2/L3 房源；L0/L1 只用于测试排除逻辑。
- 匹配总分由 Rust BFF 计算，客户端只展示。
- 线索初始状态为 `pending_assignment`，只写入临时内存；“15 分钟”是产品 SLA 提示，不表示真实顾问已收到通知。
- 未实现微信登录、短信验证码、企业实名、真实 SpacetimeDB 房源/线索写入、真实顾问分配、电话/短信/企微通知、语音、图片与文档解析。

接口与字段契约见 `docs/ai-miniapp/phase1-ai-demand-flow-api.md` 和 `docs/ai-miniapp/phase1-ai-demand-flow.openapi.yaml`。
