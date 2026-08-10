# Phase 1 AI 找房复核整改 v2 测试报告

- 测试日期：2026-08-10
- 分支：`fix/phase1-ai-demand-flow-review-v2`
- 原始 Phase 1 基线：`6e07a6c099cb65e3636ba71c4abccaac4996e5c8`
- 整改基线：`c2c57758ccf426cf50b5ef22b698312ca9b489af`
- 网络边界：未访问测试/生产服务器、SSH、SpacetimeDB、短信、对象存储或真实百炼；未启动完整根应用。

## 最终门禁

| 检查 | 命令/方式 | 最终结果 | 退出码 |
| --- | --- | --- | --- |
| Rust 编译 | `cargo check --features server` | PASS；本次模块编译通过；49 个 warning 为仓库既有/预留端口告警 | 0 |
| Rust miniapp 测试 | `cargo test --features server services::miniapp` | PASS；36 passed，0 failed，201 filtered out | 0 |
| 隔离 HTTP E2E | `cargo test --features server miniapp_http_e2e` | PASS；2 passed，0 failed，235 filtered out | 0 |
| Rust 目标格式 | `rustfmt --edition 2021 --check` + 本次 9 个 Rust 文件 | PASS | 0 |
| 前端锁文件安装 | `npm ci` | PASS；667 packages；39 个既有传递告警 | 0 |
| TypeScript | `npm run type-check` | PASS | 0 |
| 前端测试 | `npm run test` | PASS；10 files、27 tests | 0 |
| H5 构建 | `npm run build:h5` | PASS | 0 |
| 微信小程序构建 | `npm run build:mp-weixin` | PASS | 0 |
| OpenAPI | Python `yaml.safe_load` + 路径/Schema 断言 | PASS；5 paths、24 schemas | 0 |
| Critical 审计 | `npm audit --audit-level=critical` | PASS；0 critical；14 low、12 moderate、13 high | 0 |
| 空白检查 | `git diff --check` / `git diff --cached --check` | 提交前与暂存后执行并记录在复核包 | 0 |

`npm ci`、微信构建和 audit 首次在受限沙箱内遇到本机用户目录 `EPERM lstat`，经允许以同一命令在目标目录重跑后退出码均为 0；这是执行沙箱问题，不是业务回归。没有执行 `npm audit fix --force`。

E2E 开发过程中先后发现 Axum graceful-shutdown 需要显式 await、provider 断言应为 `local`、以及不可验证 fixture 必须达到 L2 推荐资格；均已修复。上表是最终代码的完整重跑结果，不隐瞒中间失败。

## Rust 覆盖

36 项 miniapp 测试覆盖：33 镇街元数据与非法镇街、中文解析和精确小数金额、类型化 hard/preference、无自由文本时货梯防绕过、用电硬/偏好、预算硬条件、数据缺失不可验证、未知自由文本硬条件、服务端重算、满足全部硬条件、幂等、未登录/过期/未确认联系方式、开发认证双门禁、动态立即/30天/90天/明确日期、跨月/跨年/闰年/非法日期、L0/L1/暂停房源排除、面积放宽、七维评分、百炼配置失败与脱敏、输入/日志安全。

## 前端覆盖

27 项测试覆盖：33 镇街完整降级、石龙/清溪/凤岗/桥头、元数据成功/失败和选择保留、30000 元与 3000000 分往返、28.5 元与 2850 分、空值、负数/超精度/超范围拒绝、页面同源 composable 的恢复顺序、首页/确认字段同步、硬条件/偏好恢复、推荐结果和线索编号恢复、v2 缓存、损坏/旧缓存清理、登录守卫、API 错误和客户端提交保护。

## 隔离 HTTP E2E

Router 绑定 `127.0.0.1:0`，只注册 5 条 miniapp 路由。Dioxus 与测试 Router 共同调用 `MiniappService`；测试配置固定 `LocalOnly`，没有可达的百炼或其他外部 adapter。真实 reqwest JSON 请求验证状态码、`application/json`、统一 envelope、snake_case、完整流程、幂等、未登录、过期、非法 JSON、硬条件不满足/不可验证、伪造客户端满足字段、伪造房源、404；graceful shutdown 后重新绑定原端口成功。

## 依赖与真实限制

审计仍报告 DCloud 精确版本工具链的 39 个非 critical 传递告警（14 low、12 moderate、13 high）。其中多项修复建议要求 `--force` 或破坏性退回不兼容版本，因此本轮没有越界升级。运行时不启用开发服务器，正式构建不启用 Mock 身份。

房源和线索仍是本地演示/内存数据；本地规则解析不是完整自然语言理解；微信登录、短信、企业实名认证、生产 SpacetimeDB、真实顾问分配/通知、预约带看、签约、支付和部署均不在 Phase 1 v2 范围。P0-01 保持 `BLOCKED`。
