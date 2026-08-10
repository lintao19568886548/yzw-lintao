# Phase 1 AI 找房垂直切片测试报告

- 测试日期：2026-08-10
- 分支：`feature/miniapp-ai-demand-flow`
- 基线：`6e07a6c099cb65e3636ba71c4abccaac4996e5c8`
- 范围：Rust/Dioxus BFF、uni-app H5、微信小程序、本地 fixture、API 契约与安全边界
- 网络边界：没有访问测试/生产服务器、SpacetimeDB、百炼、SSH、部署系统或生产数据

## 最终门禁

| 检查 | 命令/方式 | 结果 | 退出码 |
| --- | --- | --- | --- |
| Rust 编译 | `cargo check --features server` | 通过；现有工程共报告 47 个 warning，本次保留的 Spacetime 端口也产生预期 dead-code warning | 0 |
| Rust 垂直切片测试 | `cargo test --features server services::miniapp` | 21 passed，0 failed，201 filtered out | 0 |
| Rust 目标格式 | `rustfmt --edition 2021 --check`（7 个 miniapp Rust 文件） | 通过 | 0 |
| 依赖安装 | `npm install` | 通过，生成并提交 `package-lock.json` | 0 |
| TypeScript | `npm run type-check` | 通过 | 0 |
| 前端单元测试 | `npm run test` | 7 个文件、11 项测试全部通过 | 0 |
| H5 构建 | `npm run build:h5` | 通过，输出目录受 `.gitignore` 排除 | 0 |
| 微信小程序构建 | `npm run build:mp-weixin` | 通过，输出目录受 `.gitignore` 排除 | 0 |
| OpenAPI 语法 | Python `yaml.safe_load` 解析 | OpenAPI 3 文档可解析，包含 5 条路径 | 0 |
| Git 空白检查 | `git diff --check`、`git diff --cached --check` | 通过 | 0 |
| 敏感信息扫描 | 对本次新增/修改的已跟踪文件执行模式与 URL 扫描并人工复核 | 未发现 Key、Token、私钥、真实手机号、生产 IP、真实 `.env` 或业务数据 | 0 |

## Rust 覆盖

21 项测试覆盖了：中文厂房/仓库/写字楼表达、东莞镇街、面积与每平方米预算、货梯和用电；必要字段缺失；L0/L1 与暂停房源排除；无已核验房源；严格过滤；面积仅放宽 ±20%；七维得分与 0～100 总分；稳定 ID 同分排序；线索幂等；未登录、过期会话与未确认联系方式拒绝；非法 AI Provider 专用错误码；百炼缺 Key；发送模型前移除联系方式；非法 AI JSON；fixture 数量与内嵌性；顾问候选按待跟进量和稳定 ID 排序；手机号、恶意 HTML 与日志脱敏。

## 前端覆盖

11 项测试覆盖了：登录守卫、开发 Mock 边界、需求 Store、表单与手机号校验、API 业务/网络错误映射、推荐数据转换、重复提交保护，以及 Pinia 持久化恢复。测试未使用真实手机号、真实账号或外部接口。

## 浏览器冒烟

使用本地 H5 开发构建与完全本地演示配置检查了登录页：

- 桌面默认视口和 `390 × 844` 移动视口均可正常渲染。
- 可见“宜租网”“企业找房登录”“本地演示 · 非真实房源/线索”等关键文案。
- 手机号输入、联系方式确认说明和“进入 AI 找房”入口存在。
- 浏览器控制台没有 error。
- 完成后已恢复默认视口、关闭测试标签页并停止本地开发进程。

本次没有启动完整 Dioxus 应用做浏览器 API 端到端联调，因为现有根应用还包含可初始化外部服务的其他模块；在“禁止任何服务器访问”的约束下，不冒险启动该整体现有运行时。BFF 业务逻辑由 21 项 Rust 测试覆盖，客户端契约由单元测试、类型检查和双平台生产构建覆盖。

## 依赖审计

`npm audit --audit-level=critical` 退出码为 0，没有 critical。审计仍报告 39 个 DCloud 官方构建工具链的传递告警：14 low、12 moderate、13 high。安全范围内的 `npm audit fix` 已执行；剩余项只能通过 `--force` 引入破坏性版本变化或与当前 `@dcloudio/vite-plugin-uni` 的精确 Vite peer 约束冲突，因此未强制升级。运行时代码不暴露开发服务器，`manifest.json` 已关闭 uni 统计，且正式构建不启用 Mock 身份。

## 已知限制

- 房源与线索是本地演示数据；内存线索在 BFF 重启后丢失。
- 百炼适配器已实现，但无 Key 环境没有发起真实网络测试。
- 微信登录、短信验证码、企业实名认证、真实 SpacetimeDB 读写、真实顾问分配/转派、预约带看和外部通知未实现。
- 本地中文解析是确定性规则解析，不代表完整自然语言理解。
- P0-01 运行验收保持 `BLOCKED`，本切片没有修改其证据或部署配置。
