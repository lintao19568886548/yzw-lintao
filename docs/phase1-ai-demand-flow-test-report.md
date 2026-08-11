# Phase 1 AI 找房 v3 测试报告

日期：2026-08-11。所有网络监听仅使用 `127.0.0.1:0`；未访问服务器，未部署测试或生产环境。

## 仓库内最终门禁

| 命令 | 结果摘要 | 退出码 |
|---|---|---:|
| `cargo check --features server` | 通过；仅有仓库既有 warning | 0 |
| `cargo test --features server services::miniapp` | 40 passed，0 failed | 0 |
| `cargo test --features server dioxus_server_function_http_e2e` | 实际运行 1 项；1 passed，0 failed，239 filtered | 0 |
| `cargo fmt --all -- --check` | 未通过；差异位于本次未修改的历史文件（例如 billing 与 SpacetimeDB 文件） | 1 |
| `rustfmt --edition 2021 --check <7个本次修改Rust文件>` | 通过；无输出 | 0 |
| `npm ci` | 从 `package-lock.json` 安装 666 个包 | 0 |
| `npm run type-check` | `vue-tsc --noEmit` 通过 | 0 |
| `npm run test` | 12 个测试文件、41 项全部通过 | 0 |
| `npm run build:h5` | H5 `DONE Build complete` | 0 |
| `npm run build:mp-weixin` | 微信小程序 `DONE Build complete` | 0 |
| `npm audit --audit-level=critical` | 0 个 critical；仍有 13 high、12 moderate、14 low 的上游依赖告警 | 0 |
| OpenAPI strict YAML/路径/schema/枚举检查 | `OPENAPI_PARSE=PASS PATHS=5 ENUM=hard\|preference\|unspecified REFS=41` | 0 |
| `git diff --check` | 通过 | 0 |
| `git diff --cached --check` | 通过（提交前/后均检查） | 0 |

`cargo fmt --all -- --check` 的失败不是本次代码回归；根据任务约束没有批量格式化无关历史文件。本次 7 个 Rust 文件的独立格式检查退出码为 0。

## 行为回归

- 本地解析“最好有货梯”为 preference；前端显示、JSON 和服务端执行一致。
- 货梯 preference 不阻断，hard 不满足或无法核验均阻断，unspecified 完全跳过评估。
- preference→hard、hard→unspecified、刷新保持以及重复 key/非法 level 均有自动化覆盖。
- v3 缓存覆盖非法 JSON、旧版本、合法 v2 迁移、`target_towns=null`、`constraints=null`、字符串面积、非法 key/level、重复优先级、损坏 match/lead，以及确认/结果/成功三个深链读取；均未抛未捕获异常。
- 真实 Dioxus HTTP 覆盖 5 条路径和完整链路、400/404/405、认证负向、未核验过滤、硬条件、偏好、unspecified、幂等与端口释放。

```text
DIOXUS_SERVER_FUNCTION_HTTP_E2E=PASS
```

## 未消除风险

- npm 依赖树仍含 high/moderate/low 告警，但 critical 门禁为 0；强制升级会跨越 uni-app 固定版本并可能破坏构建，未在本切片擅自升级。
- P0-01 状态仍为 `BLOCKED`；本报告不是运行验收、部署证明或岗位签字。
- Phase 1 仍使用本地 fixture 与进程内存，限制详见 `docs/phase1-ai-demand-flow-known-limitations.md`。
