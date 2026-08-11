# 宜租网微信小程序 MVP

基于 uni-app、Vue 3、TypeScript 与 Pinia，支持 H5 和微信小程序。当前完成：登录、AI 找房首页、需求确认、房源推荐、房源详情、意向选择、线索确认、提交成功、我的需求和个人中心。

## 本地独立演示

```powershell
npm ci
$env:VITE_YIZU_DEMO_MODE="true"
$env:VITE_YIZU_API_BASE_URL="http://127.0.0.1:8080"
npm run dev:h5 -- --host 127.0.0.1
```

开发构建与显式演示开关同时满足时，客户端使用本地脱敏 fixture、字段邻近规则解析、确定性七维匹配、本地 session 和幂等线索，不需要启动服务器，也不会调用上述 BFF 地址。

## Rust BFF 模式

不启用 `VITE_YIZU_DEMO_MODE` 时，客户端继续使用运行配置中的 BFF 地址及五条 `/api/miniapp/v1` 接口。生产构建不会开启模拟登录，且必须配置 `VITE_YIZU_API_BASE_URL`。

百炼 Key 只能由 Rust 服务端读取，禁止放入任何 `VITE_` 变量或前端文件。

## 检查与构建

```powershell
npm run type-check
npm run test
npm run build:h5
npm run build:mp-weixin
```

微信开发者工具导入目录：`apps/yizu-client/dist/build/mp-weixin`。仓库不包含真实微信 AppID，开发者需选择测试号或配置获授权 AppID。

## 演示边界

- 所有本地房源名称、位置、图片示意与联系方式均为虚构或脱敏内容。
- L0/L1 与暂停出租房源不会进入 AI 推荐。
- 硬条件由前端选择保护并在 Rust BFF / 本地演示提交层重新核验。
- “工作时间 15 分钟联系”是产品 SLA 提示；本地演示不会通知真实顾问。
- P0-01 仍为 `BLOCKED`，不允许据此部署测试或生产环境。
