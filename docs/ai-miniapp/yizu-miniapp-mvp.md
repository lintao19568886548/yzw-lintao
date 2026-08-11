# 宜租网——企业选址与空间租赁智能服务平台

本版本面向“厂房 · 仓库 · 写字楼”完成“登录 → 描述需求 → 本地/AI解析 → 确认条件 → 房源推荐 → 详情 → 意向选择 → 线索确认 → 成功 → 我的需求”的本地可演示闭环；当前业务筛选范围仍为东莞全市。

## 双模式

- 本地演示：开发构建且 `VITE_YIZU_DEMO_MODE=true` 时启用。身份、解析、脱敏 fixture、匹配、线索和历史均在本地完成，不访问服务器。
- Rust BFF：未启用本地演示时继续使用 `/api/miniapp/v1` 现有五条接口。房源详情从匹配响应展开，我的需求首版使用本地历史缓存。

正式构建不会启用模拟登录。API 地址来自 `VITE_YIZU_API_BASE_URL`，前端不保存百炼 Key。

## 本地 H5

```powershell
cd apps\yizu-client
npm ci
$env:VITE_YIZU_DEMO_MODE="true"
$env:VITE_YIZU_API_BASE_URL="http://127.0.0.1:8080"
npm run dev:h5 -- --host 127.0.0.1
```

本地演示模式不会请求上述 BFF 地址；该变量只保留双模式配置完整性。

## 微信开发者工具

```powershell
cd apps\yizu-client
$env:VITE_YIZU_DEMO_MODE="true"
npm run build:mp-weixin
```

导入 `apps/yizu-client/dist/build/mp-weixin`。仓库不含真实微信 AppID，需要开发人员在自己的微信开发者工具中选择测试号或配置获授权 AppID。

## 隔离边界

本版本未访问测试或生产服务器，未部署，未接触生产数据、短信、对象存储、百炼或 SpacetimeDB。P0-01 仍为 `BLOCKED`。

## 2026-08-11 验收记录

- 前端：`npm ci`、类型检查、13 个测试文件 / 47 项测试、H5 构建、微信小程序构建全部通过。
- Rust：`cargo check --features server`、miniapp 41 项目标测试、Dioxus HTTP E2E 1 项测试全部通过；仅保留基线编译警告。
- H5：在 390×844 视口完整验证了登录、解析、确认、推荐、详情、意向、线索提交、成功页、我的需求和刷新恢复；浏览器错误日志为空。
- 微信产物：`dist/build/mp-weixin` 已包含 `app.json`、`project.config.json` 及完整分包。本机微信开发者工具注册记录所指的程序路径不存在，因此未做 GUI 导入；可直接导入上述构建目录。

流程截图位于 `docs/ai-miniapp/evidence/mvp-screenshots/`。
