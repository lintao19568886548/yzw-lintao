# 宜租网小程序认证与 AI 配置边界

## 模式与调用链

客户端只支持 `demo`、`test`、`production` 三种显式模式：

- `demo`：只调用浏览器/小程序内的脱敏 fixture、本地需求解析器和本地会话；不访问 BFF，不发送短信，不调用微信或百炼。
- `test`：调用 `VITE_YIZU_API_BASE_URL` 指向的测试 Rust BFF；未填写时仅回退到 `http://127.0.0.1:8080`。
- `production`：必须显式设置 `VITE_YIZU_MODE=production`，并配置 HTTPS BFF 地址；不会从 test 自动升级。

唯一允许的真实链路是：

```text
微信小程序 -> Rust API/BFF -> 微信 / 短信 / 百炼 / 业务数据适配器
```

小程序不得持有 AppSecret、短信密钥、百炼 Key、Token 签名密钥或数据库连接串，也不得直连 MySQL、SpacetimeDB 或外部提供方。

## 配置分类

当前接入：

- 微信小程序 AppID：客户端项目身份可公开配置；服务端凭证交换同时读取服务端 AppID。
- 微信 AppSecret：仅 Rust BFF 私密环境读取。
- 联麓短信：Rust BFF 复用现有 `server/spacetimedb/src/sms` 的字段、签名和模板协议，读取商户、应用、签名、版本、消息类型、模板和密钥；验证码只保存 HMAC 摘要。BFF 额外承担 IP / 设备维度限流，客户端不直连 SpacetimeDB。
- 百炼：Rust BFF 调用，失败自动回退本地解析器。
- 公开 H5 基址：客户端统一读取 `VITE_YIZU_PUBLIC_H5_BASE_URL`。

审计后决定：

- BFF 默认仍由现有 Dioxus 服务启动；本地监听应保持 `127.0.0.1`。没有反向代理、防火墙和可信代理头设计前，不配置 `0.0.0.0`。
- Access / Refresh 使用独立签名密钥；生产至少 32 字符、不得相同，并要求 `YIZU_SECRETS_ROTATED=true`。
- 现有业务数据主线仍是 SpacetimeDB。小程序房源继续使用脱敏 fixture，线索暂存仍使用当前内存仓储；本轮不接 MySQL、不连接生产数据库。

本轮暂缓并列入 Backlog：考勤自动化、招商雷达爬虫、催租短信调度、公众号凭证、Apple 账号、SSH/服务器账号、系统管理员账号、数据库 root 账号，以及未经页面路径核验的固定二维码配置。

## 本地演示

复制 `apps/yizu-client/.env.example` 为不纳入 Git 的 `apps/yizu-client/.env.local`，保持：

```env
VITE_YIZU_MODE=demo
VITE_YIZU_DEMO_MODE=true
VITE_YIZU_API_BASE_URL=
VITE_YIZU_PUBLIC_H5_BASE_URL=https://yizuw.cn
```

`.env.development.local` 也可用于 `npm run dev:mp-weixin`，但 `npm run build:mp-weixin` 的 Vite mode 是 production；当前验收统一使用 `.env.local` 以覆盖两种本地命令。构建后导入 `apps/yizu-client/dist/build/mp-weixin`。`demo` 判断不再依赖 `import.meta.env.DEV`，因此 production build 产物仍能正常启用本地演示登录。

## 真实服务配置门禁

私密值只能写入仓库已忽略的根 `.env` 或部署平台 Secret Store，不得写入任何 `VITE_` 变量。示例只保留空值。曾经通过聊天或其他公开渠道出现的值必须轮换；只有新值写入并设置 `YIZU_SECRETS_ROTATED=true` 后才可能显示 `SET`。

生产按启用项失败关闭：

- `YIZU_MINIAPP_SMS_ENABLED=true` 要求完整短信配置。
- `YIZU_MINIAPP_WECHAT_ENABLED=true` 要求微信 AppID / AppSecret。
- `YIZU_MINIAPP_AI_PROVIDER=bailian` 要求百炼 Key。
- 所有生产认证要求独立的 Access / Refresh Token 密钥。

短信验证成功后的 Access Token 使用现有 `legacy_sms` Reducer 已支持的 HS256 JWT 与 claim 命名；后续接入持久化时可由 BFF 调用该桥接建立 SpacetimeDB 会话，不另造冲突的数据库认证模型。当前小程序线索仍为内存仓储，因此本轮未连接生产 SpacetimeDB。

配置状态输出只允许 `SET`、`NOT_SET`、`INVALID`、`ROTATION_REQUIRED`，不得输出原值、前后缀或可识别长度信息。

## API 契约

```text
POST /api/miniapp/v1/auth/sms/send
POST /api/miniapp/v1/auth/sms/verify
POST /api/miniapp/v1/auth/wechat
POST /api/miniapp/v1/demands/interpret
```

短信按手机号、可信客户端 IP 和设备标识执行小时/每日限流，并有独立重发间隔。验证码由操作系统安全随机源生成，只保留带服务端密钥的 HMAC 摘要；过期、达到最大错误次数或验证成功后立即失效。

客户端已提供不挂 UI 的 `wx.login` 封装，当前短信主线不会强制触发它。微信接口只接收临时 code；openid / unionid 只采用微信服务端响应并在内部哈希后使用。code 成功交换后标记为一次性使用。AppSecret 放在 HTTPS 请求体，不进入查询参数或日志。

百炼客户端设置 5 秒连接超时、12 秒单请求超时和 20 秒总预算。输入先做长度限制和联系方式移除；任何网络、限流、超时、响应 JSON 或领域校验错误都会回退本地解析器。

## 人工验收边界

自动化测试只使用 Mock 短信、Mock 微信和 Mock 百炼；不会发送真实短信，不会产生百炼费用，不会连接生产数据库。真实外部调用必须由用户在轮换完成后另行明确开启人工验收。
