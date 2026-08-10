# 宜租网 Phase 1 AI 找房垂直切片设计

## 1. 范围与边界

本切片仅实现“企业本地演示登录 → 文字/标签/表单描述需求 → 需求结构化 → 用户确认 → 规则匹配 → 本地幂等招商线索”。不实现微信登录、真实短信、企业认证、语音、图片/文档解析、生产数据接入、真实顾问通知、部署或服务器操作。

P0-01 的运行验收证据保持原样，状态继续为 `BLOCKED`。本切片不修改 `.github/`、`deploy/`、SpacetimeDB schema、Reducer 或任何部署配置。

## 2. 仓库真实审计

### 2.1 Rust 与 Dioxus

- 根 `Cargo.toml` 是 Cargo workspace，成员为根 `parkwise`、`pure` 和 `server/spacetimedb`。
- 根应用使用 Dioxus `0.7.1` 的 `router` 与 `fullstack` 特性；入口是 `src/main.rs`，由 `dioxus::launch(app::App)` 同时承载 Web/服务端构建。
- 已有 HTTP 能力是 `src/services/` 下的 Dioxus Server Function，例如合同 AI、账单 AI、R2 和智能表接口；没有独立 Axum/Actix API 进程。
- 服务端已有 `reqwest 0.12`、可选 `tokio`、`serde`、`serde_json` 和 `dotenvy`。百炼现有调用只在 `feature = "server"` 下读取 `ALIYUN_BAILIAN_KEY`。
- 现有 Server Function 的普通单参数请求由 Dioxus 0.7 以 JSON 反序列化，普通返回值以 `application/json` 返回，因此 miniapp BFF 可复用同一个 Dioxus 服务进程并保持独立 JSON 契约。

### 2.2 认证、错误与配置

- 现有后台登录直接面向 SpacetimeDB，浏览器凭据保存在 LocalStorage；不适合作为小程序身份模型。
- 现有 Server Function 多以 `ServerFnError` 返回中文错误，没有统一 `code/message/request_id/data/errors` 包装。
- 环境变量示例集中在根 `.env.example`，本切片继续沿用该位置，只增加变量名和空值/安全默认值。
- 首轮小程序会话是独立内存会话，只在 debug 构建且 `YIZU_MINIAPP_DEV_AUTH_ENABLED=true` 时签发。release 构建即使设置该变量也拒绝模拟登录。

### 2.3 SpacetimeDB 数据模型

- 园区：`Park` 有名称、地址、状态和租户字段，但没有镇街、经纬度、核验等级或公开房源状态。
- 厂房：`Factory` 有园区关系、自营标识和描述；没有仓库/写字楼通用类型、发布状态、核验等级、消防、用电、物流等字段。
- 楼层：`FactoryFloor` 有面积、层高、承重和每平方米租金；没有可租空间单元、月租口径、货梯吨位、装卸条件或可入驻日期。
- 仓库、写字楼、通用楼栋、房间/空间单元没有可直接复用的表。
- `CompanyLead` 是外部招商雷达企业线索，要求来源 URL、公司名、证据、租户与管理员上下文，不适合存储本切片匿名/企业找房需求。
- `Investment` 是招商跟进记录，字段不足以保存需求快照、推荐房源、幂等键、渠道和分配状态。
- 未发现面向本流程的预约或带看实体。访问控制中的访客记录不等价于招商带看。

因此首轮采用 `FixtureListingRepository` 与 `InMemoryLeadRepository`，只预留 `SpacetimeListingRepository`/`SpacetimeLeadRepository` 端口，不改生产表，也不把临时线索伪装为持久化成功。

### 2.4 Node 与忽略规则

- 仓库没有 `package.json`、npm/pnpm/yarn 锁文件或根 Node workspace 约定。
- `apps/yizu-client` 独立使用 npm 并提交 `package-lock.json`，不改造根 workspace。
- 根 `.gitignore` 已排除 `node_modules/`、`dist/`、`.env`、`.env.*`、`coverage/`、`.cache/` 和常见临时文件，且保留 `.env.example`。

## 3. 模块位置与 API 映射

Rust 新模块位于 `src/services/miniapp/`，由根 `src/services/mod.rs` 引入，随现有 Dioxus Fullstack 服务启动，不创建第二个进程。路由固定为：

| 方法 | 路由 | 说明 |
|---|---|---|
| POST | `/api/miniapp/v1/auth/dev-session` | debug + 显式开关的本地会话 |
| POST | `/api/miniapp/v1/demands/interpret` | 本地或百炼结构化 |
| POST | `/api/miniapp/v1/matches` | 服务端硬过滤与七维评分 |
| POST | `/api/miniapp/v1/leads` | 幂等创建临时招商线索 |
| GET | `/api/miniapp/v1/metadata/options` | 东莞镇街、类型与表单选项 |

业务响应统一包含 `code`、`message`、`request_id`、`data` 和 `errors`。生产 Dioxus Server Function 与测试 Axum Router 均委托给同一个 `MiniappService`；隔离 Router 对不可解析 JSON 返回 400 + `INVALID_JSON` envelope，业务错误维持 200 + 明确业务码。

## 4. 领域与数据流

Rust 与 TypeScript 均使用 snake_case JSON 字段。`ConstraintKey` 类型化覆盖预算、货梯、电梯吨位、用电、消防、货车通行、装卸、分租、楼层和入驻时间，`ConstraintPriority` 用 `hard | preference` 表达级别。结果将硬条件分为已满足、未满足和数据不足无法验证；其他自由文本硬条件始终属于无法验证，不能伪装为机器已核验。

```text
uni-app 页面
  -> dev-session（仅本地）
  -> demands/interpret（不含联系方式）
  -> 用户编辑并确认 DemandDraft
  -> matches（BFF 使用脱敏 fixture，客户端不计算最终分）
  -> 用户显式点击联系顾问
  -> leads（校验会话、已确认联系方式、硬条件、推荐 ID、幂等键）
  -> InMemoryLeadRepository（进程重启可丢失）
```

## 5. AI 双模式

`DemandInterpreter` 是统一接口。

- `LocalDemandInterpreter` 完全本地、无随机数，识别类型、东莞镇街、面积、月租/单价、货梯吨位、用电、消防、入驻和常见硬/软表达；不确定字段进入 `missing_fields`。
- `BailianDemandInterpreter` 仅在 Rust server feature 编译路径调用，Key 只从 `ALIYUN_BAILIAN_KEY` 读取，模型从 `YIZU_MINIAPP_AI_MODEL` 读取；发送前去除疑似手机号，严格要求 JSON，反序列化后再做领域校验，设置超时并只重试连接/超时/429/5xx。
- 开发默认 `local`。显式 `bailian` 且缺 Key 时返回配置错误；只有 debug 构建并显式允许回退时才回退 local，响应记录 `fallback_reason`。

## 6. 匹配与线索

硬过滤依次为出租状态、AI 推荐资格、自营或 L2/L3、类型、镇街和面积。严格面积无结果时只放宽 ±20%；仍无结果仅返回“是否接受相邻镇街”的建议，不自动扩镇。提交线索时服务端重新计算以上过滤和所有类型化硬条件；未满足返回 `HARD_CONDITION_NOT_MET`，数据缺失或其他未知硬条件返回 `HARD_CONDITION_UNVERIFIED`。

七维得分按位置 20、空间 20、成本 20、生产 15、物流 10、合规 10、入驻 5 加权。核验等级、自营和更新时间只用于同分排序，稳定 ID 为最终排序键。

日期由可注入 `Clock` 提供，生产使用服务端时间换算中国标准时间，测试使用 `FixedClock`。`immediate`、30 天、90 天和明确日期均按“房源可入住日期不晚于截止日”比较，不包含任何固定 2026 当前日期。

线索必须满足：有效会话、会话内联系方式已确认、类型/镇街/面积完整、至少一个真实推荐 ID、所选房源所有硬条件满足且可验证、用户提供合法幂等键。线索初始状态为 `pending_assignment`，返回 15 分钟 SLA 和临时存储说明；不触发短信、电话、企业微信或任何外部通知。

## 7. 前端元数据、金额与恢复

- Rust `/metadata/options` 是镇街权威来源；前端 `metadata` Store 被首页和确认页共同使用，33 镇街本地列表只作完整降级，并显示非阻断提示。
- 页面只展示元；API/Pinia/Rust 保留整数分。`money.ts` 用字符串拆分整数和最多两位小数，拒绝负数、精度超限和超过 1 亿元的预算。
- 认证和需求缓存使用独立 v2 envelope。页面共同执行认证 hydrate → 需求 hydrate → composable 同步 → 守卫；确认页使用 `storeToRefs`，不持有 hydrate 前的可替换对象。

## 8. 隔离 HTTP 测试架构

`http_e2e.rs` 在测试进程内绑定 `127.0.0.1:0`，构造 `ServiceConfig::isolated_local`，因此解析路径不能读取百炼 provider 或 API Key。只注册 5 条 miniapp 路由并注入 fixture 与内存 Repository；测试完成发送 graceful shutdown，等待任务结束并重新绑定原端口证明已释放。该路径不会调用根 `main` 或初始化任何外部业务模块。

## 9. 安全设计

- 原始文字最多 1000 字，拒绝 HTML 标签/脚本样式输入；页面只使用 Vue 文本插值，不使用 `v-html`。
- 每个请求执行字段长度、集合大小、数值范围和近似序列化体积限制。
- 百炼请求不含会话 Token、手机号或其他联系方式；日志辅助函数会脱敏长数字串和 Bearer 值。
- 前端 API 基础地址由 `VITE_YIZU_API_BASE_URL` 配置；生产构建没有隐式 localhost 或模拟身份。
- Pinia 仅保存本地演示短会话和需求草稿，不保存 AI Key。
- fixture 全部虚构，不含真实门牌、业主、电话、图片 URL 或生产导出。

## 10. 当前字段映射差距

| 本切片字段 | 现有来源 | 差距/处理 |
|---|---|---|
| 园区/房源名称 | Park/Factory | 可映射，但本轮只用 fixture |
| 厂房类型 | Factory | 只有厂房；仓库/写字楼缺失 |
| 镇街 | Park.address | 非结构化，不能可信映射 |
| 可租面积/单价 | FactoryFloor | 可派生，但没有发布库存与整分租边界 |
| 自营 | Factory.is_own | 可映射 |
| 核验等级/上架状态 | 无 | 必须新增公开房源读模型后才能接入 |
| 货梯/吨位/用电/消防/物流/装卸 | 无 | fixture 覆盖，未来加法式 schema |
| 入驻日期/更新时间 | 部分有 updated_at | 没有房源可入驻日期 |
| 需求快照/推荐 IDs/幂等键/SLA | 无 | 本轮内存端口，未来新线索模型 |

## 11. 已知限制

本地中文解析器是确定性规则解析，不等同于完整自然语言理解；fixture 匹配不是生产搜索；开发会话和线索都随服务进程重启丢失；百炼实现不在无 Key 环境发起网络测试；微信登录、企业核验、顾问分配落库、管理端转派、预约/带看和真实通知均留待后续。
