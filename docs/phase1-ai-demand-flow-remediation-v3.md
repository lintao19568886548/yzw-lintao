# 宜租网 Phase 1 AI 找房 v3 二次整改

## 范围与基线

- 基线：`c323bc65e7bbbaf74cb1a9884e39b47bd0579644`。
- 分支：`fix/phase1-ai-demand-flow-review-v3`。
- 范围仅限企业演示登录、描述需求、结构化确认、房源推荐与内存线索。
- P0-01 运行验收仍为 `BLOCKED`；本次未修改该结论，也未启动 P0-02～P0-16。
- 本次未访问服务器，未连接外部业务系统，未部署测试或生产环境。

## 四项整改前后对照

| 阻塞项 | v2 | v3 |
|---|---|---|
| 优先级语义 | 仅 `hard/preference`；删除项目表示“不指定”，随后会被重新推断 | Rust、TypeScript、JSON、OpenAPI 统一为 `hard/preference/unspecified`；显式 `unspecified` 持久化并跳过评估 |
| 缓存恢复 | 只检查外层对象，嵌套损坏仍可能进入页面 | v3 key + 全对象图深层校验；任一污染整份清除；合法 v2 快照显式迁移 |
| HTTP E2E | 测试复制 5 个 Axum handler | 使用 Dioxus `ServerFunction` inventory、`register_server_functions()` 和 `FullstackState::headless()` 注册 `api.rs` 宏生成入口 |
| 复核包 | 缺少前端配置及 workspace path crate | `source/` 收录完整 Git 跟踪源码和配置，排除构建产物、密钥、数据与缓存；从全新解压目录复测 |

## 三态优先级语义

- `hard`：不满足进入 `unmet_hard_constraints`，数据不足进入 `unverified_hard_constraints`；两者都阻止线索。
- `preference`：不满足或数据不足只进入 `unmet_preferences`，不阻止线索。
- `unspecified`：字段值仍保留，但不进入硬条件或偏好评估。
- 同一 `ConstraintKey` 只能出现一次；服务端验证重复 key，Serde 拒绝未知枚举字符串。
- 新解析结果“最好有货梯”为 `preference`；“需要货梯”为 `hard`。确认页只显示最终有效枚举，不再叠加货梯特殊判断。
- 兼容缺少优先级的旧请求时，服务端保留 v2 默认：货梯 true 推断为 `hard`，其他有值字段推断为 `preference`；一旦存在显式 `unspecified`，不再回退推断。

## v2 → v3 缓存迁移

- 新 key 为 `yizu-miniapp-demand-v3`，版本号为 3。
- 合法 v2 快照必须通过 v2 完整结构校验；已有 `hard/preference` 原样保留，缺少的有值条件按 v2 默认规则物化成显式级别，然后写入 v3 并删除 v2 key。
- 无法完整校验的 v2、v3 或 v1 缓存被删除，store 回退到 `emptyDemand()`；不会 `$patch` 半份对象。
- 校验覆盖需求约束、枚举、有限数值、字符串/布尔/数组、优先级唯一性，以及 `interpretation`、`match_response`、`lead` 的页面可访问字段。

## 隔离边界

HTTP 测试只绑定 `127.0.0.1:0`，注入 `FixedClock`、本地解析器、内存会话、内存线索仓库和脱敏 fixture。它不初始化根应用、SpacetimeDB、百炼、短信、对象存储、生产数据库或生产环境变量；结束时 graceful shutdown，并在同一端口重新绑定证明释放。

最终命令、退出码和历史格式门禁说明见 `docs/phase1-ai-demand-flow-test-report.md`，真实传输证据见 `docs/phase1-ai-demand-flow-http-e2e-v3.md`。
