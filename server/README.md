# 云园慧控 SpacetimeDB 服务端

本目录是云园慧控（园区综合管理系统）的服务端。它不是传统的 REST CRUD 服务，也不是让客户端直接拼 SQL 的数据库网关，而是一个运行在 SpacetimeDB 里的 WASM 模块：数据存储、事务逻辑、权限过滤和实时推送全部在数据库内部完成，中间没有独立的应用服务器层。

> 本文讲「有什么、怎么跑」。设计原理——SpacetimeDB 基本概念详解、函数式架构（CQRS、限界上下文、共享内核）与编程范式——见 [ARCHITECTURE.md](ARCHITECTURE.md)，面向合作开发者。系统设计专题文档（[权限模型设计](../docs/权限模型设计.md)、[抽象泄露评估](../docs/抽象泄露评估.md)）统一放在仓库根目录 [docs/](../docs/)。

技术栈与规模（统计截至 2026-08）：

| 项目 | 数值 |
| --- | --- |
| Rust | 2024 Edition，crate 名 `parkwise_server`（`cdylib`，编译产物 `parkwise_server.wasm`） |
| SpacetimeDB | `2.6.1`（版本固定，本地 CLI、CI 与服务器必须一致） |
| 客户端 | Dioxus 0.7，通过 `spacetime generate` 生成的 Rust 绑定调用 |
| Table | 127 张，**全部私有**（`public` 表为 0） |
| View | 117 个，**全部 `public`**——这是客户端唯一的读取面 |
| Reducer | 269 个（含 `lifecycle.rs` 里 3 个生命周期钩子） |
| Procedure | 7 个（分页查询 3 个 + 短信 4 个） |
| 索引 | 具名 btree 索引 374 个，另有主键索引 126 个、唯一约束索引 9 个 |
| 代码量 | 约 3.04 万行，单元测试 120 个 |

> 术语说明：SpacetimeDB 的第四类接口叫 **Procedure（过程）**，不是 Producer。本文统一写 `Procedure`。

## 一、服务端负责什么

1. **业务数据存储**：园区、厂房、楼层、租户、合同、账单、财务流水、员工、考勤、请假、工资、报销、门禁、维修、电梯、消防、变压器、招商线索、组织与权限等。
2. **身份与会话**：密码登录、短信登录、刷新令牌、退出登录、租户切换、会话管理。
3. **权限隔离**：按调用者 `Identity` 解析用户、租户、角色和园区授权范围，View 只返回有权查看的数据。
4. **事务写入**：所有新增、修改、删除、审核、状态流转都通过 Reducer 在单个事务内完成。
5. **按需查询**：账单、财务、工资等大型历史列表通过 Procedure 分页查询，客户端不必长期订阅整张历史表。
6. **实时推送**：权限、会话、工作台摘要以及当前页面数据通过 WebSocket 订阅实时更新。
7. **外部服务编排**：短信发送等网络请求由 Procedure 执行（Reducer 必须保持确定性，不做 I/O）。
8. **旧系统迁移**：`reducers/platform/migration/` 下提供受管理员权限保护的 MySQL 批量迁移 Reducer。

## 二、总体架构

读写彻底分离（CQRS）：写入只有 Reducer 一个入口，读取只有 View（订阅）和 Procedure（分页）两个出口，私有表永远不直接暴露。

```text
┌──────────────────────────────────────────────────────────────┐
│                     Dioxus Web 客户端                        │
└───────────────┬──────────────────┬───────────────────────────┘
                │                  │
        WebSocket 订阅       Reducer / Procedure 调用
                │                  │
┌───────────────▼──────────────────▼───────────────────────────┐
│                      SpacetimeDB 2.6.1                       │
│                                                              │
│  Identity Token ──▶ ctx.sender() ──▶ 用户会话与租户上下文    │
│       │                                                      │
│       ├── View（117 个，public）：按身份过滤后的读取模型     │
│       ├── Reducer（269 个）：校验权限并在事务内写入          │
│       ├── Procedure（7 个）：分页查询、短信等外部 I/O        │
│       └── Table（127 张，全私有）：真实业务数据，不直接暴露  │
└──────────────────────────────────────────────────────────────┘
```

生产环境访问链路：

```text
浏览器
  -> https://yz.furong.org/v1/
  -> Nginx 反向代理
  -> SpacetimeDB 127.0.0.1:3000
  -> 数据库模块 yizu-server-yz18m
```

服务地址和数据库名通过环境变量配置，不在业务代码里写死：

```dotenv
YIZU_SPACETIMEDB_URI=https://yz.furong.org
YIZU_SPACETIMEDB_SERVER_URL=http://127.0.0.1:3000
YIZU_SPACETIMEDB_DATABASE=yizu-server-yz18m
```

CLI 侧的项目配置：

- `spacetime.json`：`server = maincloud`，`module-path = ./spacetimedb`
- `spacetime.local.json`：`database = yizu-server-yz18m`（本地覆盖）

**外部集成边界**：SpacetimeDB 模块（WASM 沙箱）不能监听入站回调（webhook）、不能维持 MQTT 等长连接、不能保存跨请求的限流状态，Procedure 只支持同步的出站请求。此类网关能力**由现有 Dioxus fullstack 进程承担**（其底层即 Axum，可挂载自定义路由与后台任务）——系统保持「`.wasm` 模块 + Dioxus/Axum 二进制」两个部署组件，不新建网关服务、不引入 Go/Java。出站异步集成（催收短信、账单推送、硬件指令、电子合同等）规划采用**事务性发件箱**模式：业务 Reducer 同事务写入 `integration_task` 任务表，网关经订阅获取任务、调用外部系统后由专用 Reducer 幂等写回。仅当集成流量与页面渲染出现资源争用时，才按预案在同 workspace 内拆出独立二进制。决策依据、任务表设计要求及 crate 选型见 [ARCHITECTURE.md](ARCHITECTURE.md) §2.10。

## 三、业务域划分

`tables/`、`views/`、`reducers/` 三个目录用同一套业务域名字对齐切分，一个功能的三层代码永远在同名子目录里：

| 业务域 | Table | View | Reducer | 内容 |
| --- | --- | --- | --- | --- |
| `platform` | 46 | 45 | 92 | 平台底座：用户/客户/组织/会话（`center`）、媒体图片（`media`）、导航菜单（`navigation`）、公告（`notices`）、角色权限（`permissions`）、支持工单（`support`）、系统配置（`system`）；Reducer 侧另有 `migration`（MySQL 迁移） |
| `rental` | 12 | 12 | 36 | 租赁：园区/厂房/楼层等资产（`assets`）与租户主档、合同（`contract`） |
| `hr` | 15 | 9 | 35 | 人事：员工、考勤、请假、工资及附件、工资-流水关联（`employee`），用户角色关系（`relations`） |
| `finance` | 8 | 9 | 19 | 财务：收支流水（`record`）、总账单/水费/电费/催收（`billing`）、报销审批（`approvals`） |
| `investment` | 15 | 15 | 36 | 招商：线索（`lead`）、评分（`score`）、信号（`signal`）、爬虫（`crawler`）。**目前没有任何前端页面使用** |
| `maintenance` | 20 | 15 | 22 | 维护：维修工单为流程流水；消防、变压器、电梯均为资产与巡检分离的台账（见 [docs/变压器台账与扫码巡检.md](../docs/变压器台账与扫码巡检.md) 及第五、六篇） |
| `access_control` | 3 | 3 | 10 | 门禁：车辆、访客登记等 |
| `shared` / `workbench` | — | 3 | 1 | 共享 View（`current_user`、`current_center_user`）、工作台聚合（`dashboard_overview`）、引导 Reducer |

跨域写入被刻意压到极少数几处，且全部显式写在代码里（见「六、项目约定」）。

## 四、四类服务端接口

### 1. Table：真实数据表（全部私有）

`tables/` 下定义业务表、主键、索引和关联字段。**127 张表全部是私有表**——客户端不能订阅任何一张表，只能订阅 View。这不是偷懒的默认值，而是权限模型的地基：私有表 + 身份过滤 View 意味着「忘记设权限」这种事故在结构上不可能发生。

```rust
#[spacetimedb::table(
    accessor = salary_finance,
    index(accessor = salary_finance_by_salary, btree(columns = [salary_id]))
)]
pub struct SalaryFinance {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub salary_id: u64,
    pub finance_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
```

设计约束：

- 客户端不能直接写表，也不允许用 SQL 绕过 Reducer 改数据。
- 每张表都带租户归属（`customer_id`）、审计时间（`created_at` / `updated_at`），业务表普遍带软删除（`is_deleted`）。
- 关联数据必须在同一个 Reducer 事务内一起更新，不留半完成状态。
- 列的修改与删除无法平滑迁移，表的**列设计须一次定稿**；索引可以事后增删（见第七节）。

### 2. View：按身份过滤的读取模型（全部 public）

`views/` 下的 117 个 `public` view 是客户端唯一的读取边界，统一用 `my_*` 命名。View 内部从 `ctx.sender()` 解析出当前用户的 `ReadScope`，逐行过滤后返回：

```rust
#[spacetimedb::view(accessor = my_finances, public)]
fn my_finances(ctx: &ViewContext) -> Vec<Finance> {
    let Some(scope) = read_scope(ctx) else { return Vec::new() };
    ctx.db.finance().iter()
        .filter(|row| !row.is_deleted && scope.allows_park(row.park_id))
        .collect()
}
```

常用 View 一览（均已在前端使用）：

| 类别 | View |
| --- | --- |
| 身份 | `current_user`、`current_center_user` |
| 工作台 | `dashboard_overview`（服务端聚合的首页指标） |
| 租赁 | `my_parks`、`my_factories`、`my_factory_floors`、`my_rental_tenants`、`my_tenant_profiles` |
| 财务 | `my_finances`、`my_amount_bills`、`my_ele_bills`、`my_water_bills`、`my_collection_sms_logs`、`my_finance_images` |
| 人事 | `my_employees`、`my_attendances`、`my_leave_applications`、`my_salaries`、`my_reimbursements`、`my_attendance_locations` |
| 门禁 | `my_access_cars`、`my_access_visitors` |
| 维护 | `my_repair_orders`、`my_elevators`、`my_firefighting_records`、`my_transformers` |

规矩：**不允许为了方便新建「返回全部数据」的 View**。新增 View 的第一步是定义它的权限边界（用哪个 scope 方法过滤）。

### 3. Reducer：事务性业务写入

Reducer 是唯一的命令入口。执行链路：

```text
客户端调用 Reducer
  -> SpacetimeDB 验证 Identity Token
  -> ctx.sender() 取得调用者身份
  -> 校验登录状态、角色、租户、园区权限（AdminContext::require 等）
  -> 校验业务参数与状态流转（validated_* 纯函数）
  -> 在一个事务中写入一张或多张表
  -> 提交成功后自动推送所有受影响的订阅
```

约束：

- **确定性**：不做网络请求、不读文件、不用系统时钟和随机数——时间用 `ctx.timestamp`，随机用 `ctx.rng()`。
- **鉴权在服务端**：客户端传入的用户 ID、角色 ID、园区 ID 一律不作为授权依据，身份只认 `ctx.sender()`。
- **错误即文案**：业务错误返回 `Err(String)`，中文、可直接展示给用户（如「已发放的工资必须填写金额」）。
- **校验抽纯函数**：参数校验、状态机判断抽成不依赖 `ctx` 的纯函数，用中文函数名写单元测试（现有 120 个）。
- 批量导入类 Reducer 限管理员调用，失败整批回滚。

### 4. Procedure：按需查询与外部 I/O

Procedure 是请求/响应入口，承担两类工作：Reducer 做不了的外部网络请求，以及不适合长期订阅的大列表分页。当前全部 7 个：

| Procedure | 文件 | 作用 |
| --- | --- | --- |
| `query_billing_page` | `procedures/history.rs` | 账单历史分页 |
| `query_finance_page` | `procedures/history.rs` | 财务流水历史分页 |
| `query_salary_page` | `procedures/history.rs` | 工资历史分页 |
| `send_login_sms_code` | `procedures/center/auth/sms.rs` | 发送登录短信验证码 |
| `login_with_sms_code` | `procedures/center/auth/sms.rs` | 校验验证码并登录 |
| `send_collection_sms` | `procedures/billing/collection.rs` | 批量催收短信 |
| `send_contract_reminder_sms` | `procedures/rental/reminder_sms.rs` | 合同到期提醒短信 |

与 Reducer 的分工：

| 对比项 | Reducer | Procedure |
| --- | --- | --- |
| 职责 | 修改业务状态 | 按需查询、外部 I/O |
| 确定性 | 必须确定，纯事务 | 可发 HTTP 请求 |
| 典型场景 | 增删改、审核、状态流转 | 分页历史、短信 |
| 客户端拿到 | 成功 / 业务错误 | 结构化结果（一次性） |

历史大表走 Procedure 而不进常驻订阅，是为了压低登录耗时、浏览器内存和缓存刷新压力。短信相关的签名加密和供应商对接在 `sms/`（`crypto.rs`、`provider.rs`）。

## 五、身份、租户与权限模型

权限内核在 `src/access.rs`，被所有 View 和 Reducer 共用。系统不信任客户端传的任何 ID，身份链条从连接令牌开始：

```text
Identity Token
  -> ctx.sender()
  -> 用户会话 -> 当前用户 -> 当前租户（customer）
  -> 角色 / 权限码 / 园区授权
  -> Principal（是谁）+ ReadScope（能看什么）
```

关键类型与函数：

| 项 | 说明 |
| --- | --- |
| `Principal` | 当前调用者的完整上下文；`is_admin()` 判断超管（角色名常量 `ADMIN_ROLE_NAME = "Super"`） |
| `ReadScope` | 读取范围。`is_unrestricted()` 不受园区限制；`allows_park(park_id)` 判断某园区行是否可见；`has_code("hr:manage")` 判断权限码；`has_duty(Duty)` 判断职能；`owns_or_has_duty(...)` 归属或职能二选一 |
| `allows_optional_park` | 专门处理挂在 `NO_PARK`（0）下的行：**只有不受园区限制的账号可见**。工资支出等公司层面流水记在 `NO_PARK` 下，受限账号在财务里看不到 |
| `read_scope(ctx)` | View 里第一行调用，拿不到就返回空集 |
| `can_access_path` / `visible_parks` / `can_access_park` | 菜单路径与园区级别的可见性判断 |
| `reducers/shared/access.rs` | Reducer 侧的门卫：`AdminContext::require(ctx)?`、`current_customer_id`、各类 `require_*`（找不到即返回中文错误） |

必须遵守：

- View 负责读隔离，Reducer 负责写授权，两边都不能省。
- 超管能力显式判断（`is_admin()`），不能靠「查不到限制记录」隐式放行。
- 租户切换后客户端必须重建最小订阅集。

## 六、项目约定

这些约定贯穿全部 127 张表和 269 个 Reducer，改代码前先读一遍：

1. **金额一律整数分**：所有金额字段是 `i64`、后缀 `_cents`。全服务端表结构里浮点字段为 0 个——钱不进浮点。
2. **时间一律 `spacetimedb::Timestamp`**：不用字符串存时间（仅存量遗留 3 个 `Option<String>` 字段）。
3. **园区外键一律 `u64` + `NO_PARK`（0）哨兵**，不用 `Option<u64>`。原因写在 `reducers/shared/park_ref.rs` 的模块文档里：SpacetimeDB 的索引过滤参数必须实现 `FilterableValue`，`Option<u64>` 不在其中——带 `Option` 的列即使建了索引也只能全表扫。这是被数据库逼出来的选择，不是偏好。
4. **软删除**：业务删除置 `is_deleted = true`，物理删除仅用于附件关联等纯关系行。财务流水的「冲销」就是软删。
5. **模块注册陷阱**：文件内 `mod` 名如果与表的 `accessor` 同名，会遮蔽宏生成的访问器导致编译错误。约定写法：`#[path = "salary.rs"] mod salary_table;`。
6. **跨域写入显式且极少**：全服务端只有少数几处跨域写，全部是「业务事实 → 财务流水」方向：
   - 账单创建 → 插入一笔「账单收入」流水（`reducers/finance/billing/amount.rs`）；
   - 报销审批通过 → 插入一笔「其他费用/支出」流水（`reducers/finance/approvals/reimbursement.rs`）；
   - 工资标记发放 → 经 `salary_finance` 关联表插入一笔「工资支出」流水，改金额同步、取消发放软删冲销（`reducers/hr/employee/salary.rs`）。
   三处口径一致：记账靠插入，退回靠 `is_deleted` 软删。
7. **扩展已有表：优先表尾追加带默认值标注的列**（整数、布尔、`Option<T>`）；需要非空字符串列、或关系本身是一对多时用关联表（`salary_finance` 即关联表范例）。修改、删除既有列不可行（见下一节）。
8. **单元测试中文命名**，测的是抽出来的纯函数，如 `已发放的工资必须有金额()`。
9. **纯函数显式标注 `#[pure_function::pure]`**（自制属性宏，crate 在仓库根 `pure/`）：宏在编译期拒绝函数里出现 `ctx`、`Signal`、Hook 等副作用入口。服务端 `check_*` / `normalize_*` 与前端页面 `model.rs` 的公开函数必须标注，漏标会被两侧的完备性测试点名（需要 `ctx` 的函数改名 `require_*` / `validated_*`；前端读时钟的函数以 `now_` / `today` 开头命名豁免）。详见 ARCHITECTURE.md §3.8。

## 七、Schema 演进规则（2.6.1 实测）

以下行为在 SpacetimeDB 2.6.1 上**逐项实验验证**（方法：本地探针模块，建表插入数据 → 实施变更 → 不带 `--delete-data` 发布 → 核对数据；结论与官方 Automatic Migrations 文档相互印证）：

| 变更类型 | `spacetime publish` 行为 | 数据 |
| --- | --- | --- |
| 新增表 / Reducer / View / Procedure | 自动迁移（日志 `Database updated`） | **保留** |
| 已有表新增 / 移除索引 | 自动迁移 | **保留** |
| 表尾追加**带 `#[default(...)]` 标注**的列 | 自动迁移，存量行填入默认值 | **保留** |
| 追加**无默认值标注**的列 | 拒绝发布（`requires a default value annotation`）；仅 `--delete-data` 可强制 | 强制则**全库清空** |
| 修改 / 删除已有列（改类型、改名等） | 拒绝发布；仅 `--delete-data` 可强制 | 强制则**全库清空** |
| 删除表；给已有列加 Unique / 主键约束 | 官方文档列为禁止（未逐项实测） | — |

`#[default(...)]` 标注的实测注意事项：

- 整数字面量必须带类型后缀：`#[default(0u64)]`。写 `#[default(0)]` 会按 32 位宽度序列化，发布时服务端校验失败；
- `Option` 列写 `#[default(None::<String>)]`——`None` 必须用 turbofish 标注类型，否则编译不过；
- 非空 `String` 列无法声明默认值（默认值在 const 上下文求值，带析构器的类型不可用，编译错误 E0493）。**要追加字符串列，用 `Option<String>`。**

由此的四条规则（依实测于 2026-08 修订；此前版本称「加索引、加列一律清库」，经实验证伪并更正）：

1. **扩展已有表优先追加带默认值标注的列**（整数、布尔、`Option<T>`）；需要非空字符串、或关系本身是一对多时，用关联表（`salary_finance` 即关联表范例）。
2. **修改、删除既有列依旧等于清库**，此类需求必须重新设计（追加新列、关联表或导出重建）。
3. **索引可以事后增删**，无需建表时穷举。
4. **已由最新决策替代：**生产发布永久禁止清库和重建，工作流及远端脚本不再接受数据删除变量。遇到不兼容变更必须重新设计为兼容迁移，不存在临时打开生产清库开关的例外。

发布前自查两个问题：这次改动是否**修改或删除**了已有列？是否追加了**没有默认值标注**的列？任一为是，就停下来重新设计。

## 八、目录结构

```text
server/
├── AGENTS.md            # SpacetimeDB 概念、CLI、Rust SDK 速查（面向 AI 协作）
├── ARCHITECTURE.md      # 架构与编程范式指南（给合作开发者）
├── README.md            # 本文
├── spacetime.json       # CLI 项目配置：server = maincloud，module-path = ./spacetimedb
├── spacetime.local.json # 本地覆盖：database = yizu-server-yz18m
└── spacetimedb/
    ├── Cargo.toml       # crate: parkwise_server，cdylib
    └── src/
        ├── lib.rs           # 模块入口：access / lifecycle / procedures / reducers / sms / tables / views
        ├── lifecycle.rs     # init（首次发布引导）/ client_connected / client_disconnected
        ├── access.rs        # 权限内核：Principal、ReadScope、Duty、权限码
        ├── tables/          # 127 张私有表：platform、rental、hr、finance、investment、maintenance、access_control
        ├── views/           # 117 个 public View：上述七域 + shared + workbench
        ├── reducers/        # 245 个业务 Reducer：七域 + shared 共享内核
        │   └── shared/      #   access.rs（门卫）、bootstrap.rs（管理员初始化）、
        │                    #   park_ref.rs（NO_PARK 哨兵）、validation.rs（文本校验）
        ├── procedures/      # 7 个 Procedure：history.rs（分页）、center/auth（短信登录）、
        │                    #   billing（催收）、rental（合同提醒）
        └── sms/             # 短信实现：crypto.rs（签名加密）、provider.rs（供应商）
```

三层目录用同一套域名对齐：改「工资」就去 `tables/hr/employee/`、`views/hr/`、`reducers/hr/employee/` 三处，不会散落别处。

## 九、本地开发

以下命令均从**仓库根目录**执行。

### 安装并固定 SpacetimeDB 版本

```bash
curl -sSf https://install.spacetimedb.com | sh -s -- --yes
spacetime version install 2.6.1
spacetime version use 2.6.1
```

若 `spacetime` 指向了别的版本，可直接用固定版本的 CLI：

```bash
~/.local/share/spacetime/bin/2.6.1/spacetimedb-cli --version
```

### 构建与测试

```bash
# 编译 WASM 模块（产物：target/wasm32-unknown-unknown/release/parkwise_server.wasm）
spacetime build --module-path server/spacetimedb

# 服务端单元测试（120 个）
cargo test --manifest-path server/spacetimedb/Cargo.toml
```

### 生成客户端绑定

改了 Table、View、Reducer、Procedure 或它们的输入输出类型后，必须重新生成：

```bash
spacetime generate \
  --lang rust \
  --module-path server/spacetimedb \
  --out-dir src/spacetime_bindings \
  --yes
```

注意三点（都踩过坑）：

- 输出目录是 `src/spacetime_bindings/`，**不是** `src/module_bindings/`；
- 参数是 `--module-path`，**不是** `--project-path`；
- 私有表同样会生成行类型文件——生成物只该是新增/修改对应类型，出现大面积删除说明命令跑错了。

`src/spacetime_bindings/` 是生成产物，不要手工修改。

### 前端联调

```bash
dx serve          # 本地开发
dx bundle --release   # 发布构建
```

## 十、发布与 CI/CD

### 生产发布（唯一正规途径）

P0-01 后，`.github/workflows/deploy-production.yml` 仅允许手动触发，推送 `main` 不会触发生产部署。工作流保留两个 Job；完整的受保护审批和 artifact 门禁由 P0-16 实施：

1. **Bundle Dioxus + build SpacetimeDB**：检出、装 Rust / Dioxus CLI / SpacetimeDB CLI 2.6.1、校验生产端点、`dx bundle --release`、编译 WASM、打包上传部署产物。
2. **Publish Dioxus + SpacetimeDB**：SSH 同步到服务器、生成运行时环境变量、执行远端部署脚本（更新 Web 服务并 `spacetime publish` 模块）。

生产工作流和远端脚本均不接受清库变量、任意发布命令或强制发布入口。若 schema 不兼容，固定发布命令必须失败并停止。

### 发布后核对（每次都做）

```bash
gh run watch <run-id> --exit-status                # 盯到 CI 结束
SPACETIME=~/.local/share/spacetime/bin/2.6.1/spacetimedb-cli
$SPACETIME logs -s https://yz.furong.org yizu-server-yz18m -n 200   # 无 ERROR
$SPACETIME sql  -s https://yz.furong.org yizu-server-yz18m \
  "SELECT park_name FROM park LIMIT 3"             # 抽查数据仍在（确认没清库）
curl -o /dev/null -s -w '%{http_code}\n' https://yz.furong.org/    # 前端 200
```

### 测试环境发布

```bash
spacetime publish --server local --module-path server/spacetimedb yizu-server-yz18m
```

不清楚 Schema 兼容性时，禁止任何带 `--delete-data` 的命令（判断标准见第七节）。

## 十一、MySQL 迁移

旧 MySQL 系统在迁移阶段仍是历史数据与隐式关系的事实来源；运行中的新业务以 SpacetimeDB 为准。迁移入口在 `reducers/platform/migration/`（`billing.rs`、`factory.rs`、`media.rs`、`salary.rs`、`sequence.rs`），全部要求管理员身份。

原则：

1. 先迁依赖数据（园区、租户），再迁账单、财务、工资。
2. 只走迁移 Reducer，不碰 SpacetimeDB 存储文件。
3. 保留旧系统主键或建立显式 ID 映射（`sequence.rs`）。
4. 日期、布尔、`Option` 转成 SATS 认识的类型；金额转整数分。
5. 图片先上对象存储，再登记图片表并回填业务关联。
6. 批次失败整批回滚，不留半截数据。
7. 导入后用 View / 分页 Procedure 核对数量、金额、关联关系。

## 十二、常见问题

### 为什么表全部私有、View 全部公开？

`public` 对表意味着「任何登录客户端都能订阅整张表」，对 View 只意味着「可订阅」——过滤逻辑在 View 函数体内按 `ctx.sender()` 执行。表全私有 + View 全公开，等于把「每条读取都过权限」变成结构性事实，而不是靠自觉。

### 为什么客户端不能直接写 SQL？

直接写 SQL 绕过了权限校验、业务校验、状态机和多表事务。所有写入必须走 Reducer。

### 为什么不是所有数据都走 WebSocket 订阅？

账单、财务、工资历史量大，全量常驻订阅会拖慢登录、吃浏览器内存、放大缓存刷新。它们走 `query_*_page` 分页 Procedure；实时订阅留给权限、会话、摘要和当前页面数据。

### Reducer 里能发短信吗？

不能。Reducer 必须确定且纯事务；短信等外部请求一律放 Procedure（见 `procedures/center/auth/sms.rs` 等）。

### 加个字段会清库吗？

分情况：表尾追加**带 `#[default(...)]` 标注**的列可以平滑发布、数据保留；追加无默认值的列、修改或删除已有列则只能 `--delete-data`（全库清空）。索引增删是安全的。详见第七节。

### 连接成功但看不到数据？

依次检查：

1. Identity Token 是否属于正确用户；
2. 当前租户（customer）是否正确；
3. 用户是否有角色和权限码；
4. 用户或角色是否分配了对应园区——注意挂在 `NO_PARK` 下的行（如工资支出）只有不受园区限制的账号可见；
5. 客户端订阅的是不是正确的 `my_*` View；
6. 该页面是否本来就该走分页 Procedure 而不是订阅。

### 服务端接口改了，客户端没有对应方法？

重新生成绑定：

```bash
spacetime generate --lang rust --module-path server/spacetimedb --out-dir src/spacetime_bindings --yes
```
