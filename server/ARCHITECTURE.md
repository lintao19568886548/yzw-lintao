# 云园慧控服务端架构说明书

**作者**：周茂森

**面向对象**：参与本项目服务端开发的协作人员。本文不假设读者具备 SpacetimeDB 使用经验，所有平台专有名词均在首次出现处给出定义；但假设读者具备 Rust 语言基础。

**文档分工**：

| 文档 | 内容 |
| --- | --- |
| 本文（ARCHITECTURE.md） | 平台概念、架构设计原理、编程规范、开发流程 |
| [README.md](README.md) | 系统规模、目录结构、构建与部署命令、生产环境信息 |
| [AGENTS.md](AGENTS.md) | SpacetimeDB CLI 与 Rust SDK 的 API 速查手册 |
| [docs/](../docs/) | 系统设计专题文档：[权限模型设计](../docs/权限模型设计.md)、[抽象泄露评估](../docs/抽象泄露评估.md) |

**全文结构**：第一章介绍 SpacetimeDB 平台的基本概念；第二章说明本项目的总体架构及其设计依据；第三章规定编码规范；第四章规定开发流程。第一章为后续各章的前置知识，建议按顺序阅读。

---

## 第一章　SpacetimeDB 平台概念

### 1.1 概述：数据库即应用服务器

传统 Web 应用采用三层结构：客户端、应用服务器（承载业务逻辑）、数据库（承载数据）。SpacetimeDB 的核心目的是**将「应用服务器」与「数据库」两层合并为一层，从而消除二者之间的网络往返与一致性同步问题**：业务逻辑以 WebAssembly（WASM）模块的形式部署在数据库内部执行，客户端直接连接数据库。官方对其定位的表述是：一个功能完整的关系型数据库系统，允许将应用逻辑直接运行于数据库内部，不再需要单独部署 Web 服务器或游戏服务器；模块可用 Rust、C#、TypeScript、C++ 四种语言编写（2.0 版起，后两者为新增），鉴权等服务端逻辑仍按传统服务器的方式编写。

两种结构的对比如下：

| 对比项 | 传统三层结构 | SpacetimeDB |
| --- | --- | --- |
| 业务逻辑位置 | 独立的应用服务器进程 | 数据库内部的 WASM 模块 |
| 客户端与数据的通信 | HTTP 请求 → 应用服务器 → SQL → 数据库 | WebSocket 直连数据库 |
| 数据读取方式 | 请求-响应（轮询获取更新） | 订阅-推送（服务端主动推送变更） |
| 业务逻辑与数据的事务边界 | 需应用层自行管理（ORM、事务注解等） | 天然一致：逻辑在数据库事务内执行 |
| 部署单元 | 应用程序 + 数据库两套设施 | 单个 WASM 模块 |

**被取代的技术栈**。以传统架构的组成部分逐层对照，SpacetimeDB 所取代的对象包括：

1. **应用服务器层**：Node、Django、Rails、Spring 等承载业务逻辑的独立进程，由「运行在数据库内部的 WASM 模块」取代。
2. **ORM 层与连接池**：逻辑与数据处于同一进程，不再存在跨进程的序列化与反序列化，此二者随之消失。
3. **缓存层**：传统架构中为降低数据库压力而引入的 Redis 等缓存设施，由「客户端本地订阅缓存 + 服务端增量推送」取代（机制见 §1.8）。
4. **事件总线与消息队列**：用于「数据变更后通知相关方」的发布-订阅系统（Kafka 等），由平台原生的订阅（Subscription）机制取代，无需自行搭建设施实现最终一致性同步。
5. **部署运维设施**：官方将其概括为免除微服务、容器、Kubernetes、Docker、虚拟机及 DevOps 流程等一整套运维基础设施——部署单元只有一个 WASM 模块。
6. **手写的实时同步代码**：这是该平台最初的核心动机（见下段）。

**设计渊源**。SpacetimeDB 由 Clockwork Labs 开发，脱胎于其自研 MMORPG《BitCraft》的工程实践：游戏后端过去需要手工编写权限校验、物理逻辑、状态同步等「有状态的事务性云函数」，并自行维护 WebSocket 连接实现实时数据推送。SpacetimeDB 将这些需求整合为两个原语——归约器（Reducer，§1.4）与订阅查询（§1.8）。官方 FAQ 亦将其与 Mirror、Photon 等传统游戏联网库对比：后者仅解决「客户端与服务端之间如何传递消息」，服务端逻辑、状态管理与持久化仍需自行编写与部署；SpacetimeDB 取代的是整个服务端。

**许可与选型风险**。SpacetimeDB 以 Business Source License 1.1 发布（约定若干年后转为附带链接例外条款的 AGPL v3），并非完全开源许可，代码治理集中于 Clockwork Labs 一家公司——采用该平台须将此项平台锁定风险纳入考量。目前唯一公开的大规模生产案例是 Clockwork Labs 自营的《BitCraft Online》，属高频状态同步的游戏场景，与本项目的管理后台场景差异较大，其验证价值应审慎评估。官方宣称的性能数据（部分基准较传统数据库快百倍至千倍）出自厂商自测，无第三方基准佐证，引用时应保持保留态度。

**与 BaaS 平台的区别**。Firebase、Supabase 等后端即服务（Backend as a Service）平台本质上仍是「数据库 + 一层薄 API」，复杂业务逻辑须编写于云函数或边缘函数中，受冷启动与执行时长限制约束；SpacetimeDB 则将整个应用（包括复杂业务逻辑）以真正的编程语言写成单个模块，直接运行于数据库进程内，不存在冷启动与执行时长限制。

概括而言：该平台最初为解决游戏行业「实时多人状态同步」这一高频难题而生，其后延伸为一项通用主张——**以单个数据库进程，取代「应用服务器 + ORM + 缓存 + 消息总线 + 部署运维」的整套传统技术栈**。

本项目是上述主张的直接实践：服务端即 `server/spacetimedb/` 目录下的单个 Rust crate（crate 名 `parkwise_server`），编译产物为 `parkwise_server.wasm`，通过 `spacetime publish` 命令部署至数据库实例。系统中不存在 HTTP 路由层、ORM 层、连接池与缓存层。

客户端与服务端的全部交互仅有三种形式，分别对应后文将介绍的三类接口：

1. **订阅 View**——读取数据；
2. **调用 Reducer**——写入数据；
3. **调用 Procedure**——执行分页查询或外部输入输出操作。

### 1.2 术语总表

下表汇总本文使用的平台术语。各术语的详细说明见后续小节。

| 术语 | 英文 | 定义 |
| --- | --- | --- |
| 模块 | Module | 部署在数据库内部的 WASM 程序，包含全部业务逻辑与数据定义 |
| 数据表 | Table | 持久化数据的存储结构，由 Rust 结构体声明（§1.3） |
| 归约器 | Reducer | 在数据库内部以事务方式执行的状态变更函数，系统唯一的写入入口（§1.4） |
| 视图 | View | 依据调用者身份实时计算结果的只读函数，系统唯一的订阅读取面（§1.5） |
| 过程 | Procedure | 以请求-响应方式调用、允许执行外部网络操作的函数（§1.6） |
| 身份 | Identity | 用户的全局唯一标识，由连接令牌确定，服务端经 `ctx.sender()` 获取（§1.7） |
| 订阅 | Subscription | 客户端向服务端注册查询、持续接收结果集变更的机制（§1.8） |
| 归约器上下文 | ReducerContext | Reducer 执行时可访问的环境对象，提供数据库句柄、调用者身份、事务时间戳等（§1.9） |
| 客户端绑定 | Client Bindings | 由 CLI 依据模块定义自动生成的类型安全客户端代码（§1.11） |

### 1.3 Table（数据表）

**定义**：Table 是 SpacetimeDB 中持久化数据的存储结构，功能上对应关系数据库中的表。它以带有 `#[spacetimedb::table]` 属性宏的 Rust 结构体声明，结构体的每个字段对应表的一列。

声明示例（摘自本项目 `tables/hr/employee/salary_finance.rs`）：

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

其中各要素的含义：

- `accessor`：表的访问器名称。模块代码经 `ctx.db.salary_finance()` 形式访问该表；
- `index(...)`：B 树索引声明。声明后可使用 `ctx.db.salary_finance().salary_finance_by_salary().filter(值)` 进行索引查找；
- `#[primary_key]`：主键约束；`#[auto_inc]`：自增列，插入时以 0 为占位值，由数据库分配实际值；
- 列类型支持整数、布尔、字符串、`Timestamp`、`Identity`、`Option<T>`、`Vec<T>` 等（完整列表见 AGENTS.md）。

**可见性**：Table 分为公开（`public`）与私有（默认）两种。公开表允许任何已连接客户端订阅其全部行；私有表仅模块内部代码可以访问，客户端无法以任何方式直接读取。

**本项目的约束**：全部 127 张表均为私有表。客户端读取数据的唯一途径是订阅 View（§1.5）。该决策的架构意义见 §2.3。

### 1.4 Reducer（归约器）

**定义**：Reducer 是在数据库内部以事务方式执行的函数，是修改数据库状态的唯一入口。客户端不能直接写表，亦不能执行 SQL 写语句；一切数据变更——新增、修改、删除、审核、状态流转、批量导入——都必须通过调用某个 Reducer 完成。

**名称由来**：「Reducer」一词源于函数式编程中的归约（reduce / fold）操作——即「以当前状态与一项输入为参数、计算出下一个状态」的函数（前端领域的 Redux 框架对该词的使用与此同源）。SpacetimeDB 沿用此命名，因为每个 Reducer 的本质正是：**以当前数据库状态与调用参数为输入，产出下一个数据库状态**。

声明示例（摘自本项目 `reducers/hr/employee/salary.rs`）：

```rust
#[spacetimedb::reducer]
pub fn create_salary(ctx: &ReducerContext, input: SalaryInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_salary(ctx, 0, customer_id, input)?;
    let row = ctx.db.employee_salary().insert(row);
    sync_salary_finance(ctx, &row);
    Ok(())
}
```

Reducer 具有四项关键性质：

1. **事务性**：一次 Reducer 调用构成一个数据库事务。函数正常返回则调用内的全部表操作原子提交；返回 `Err` 或发生 panic 则全部回滚。跨多张表的写入天然具备原子性（详见 §1.10）。
2. **确定性**：Reducer 内部不得访问系统时钟、随机源、文件系统与网络。时间与随机数必须取自上下文对象（详见 §1.9 与 §1.12）。
3. **不向调用方返回数据**：Reducer 的返回类型为 `Result<(), String>`（或无返回值）。调用方仅能得知执行成功或失败及失败原因，**不能获得查询结果**。数据始终经由订阅送达客户端：写入提交后，受影响的订阅自动收到变更推送。此性质并非限制，而是读写分离结构的组成部分（见 §2.3）。
4. **服务端鉴权**：Reducer 在执行业务操作前自行校验调用者的身份与权限。客户端界面上隐藏某个按钮不构成任何安全保证。

**生命周期 Reducer**：除业务 Reducer 外，存在三个由平台在特定时机自动调用的钩子，本项目定义于 `src/lifecycle.rs`：`init`（模块首次发布或更新后执行，本项目在此完成管理员账号初始化）、`client_connected` 与 `client_disconnected`（客户端连接建立与断开时执行）。

**执行链路**：一次完整的 Reducer 调用依次经过以下环节：

```text
客户端调用 Reducer
  → SpacetimeDB 验证连接的 Identity Token
  → Reducer 经 ctx.sender() 取得调用者身份
  → 校验登录状态、角色、租户、园区权限（本项目：AdminContext::require 等门卫函数）
  → 校验业务参数与状态流转（本项目：validated_* 纯函数）
  → 在同一事务内写入一张或多张表
  → 事务提交
  → 平台向所有受影响的订阅推送增量变更
```

### 1.5 View（视图）

**定义**：View 是依据调用者身份实时计算返回结果的只读函数。它与关系数据库中「视图」概念的区别在于两点：其一，View 的计算过程隐式地以**当前调用者的身份**为参数，同一个 View 对不同用户返回不同结果；其二，View 可以被**订阅**——客户端注册订阅后，每当底层数据变更导致该 View 的结果集变化，变更将被自动推送（见 §1.8）。

声明示例（摘自本项目 `views/finance/record.rs`）：

```rust
#[spacetimedb::view(accessor = my_finances, public)]
pub fn my_finances(ctx: &ViewContext) -> Vec<Finance> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut finances = ctx
        .db
        .finance()
        .finance_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|f| !f.is_deleted && scope.allows_park(f.park_id))
        .collect::<Vec<_>>();
    finances.sort_by_key(|f| f.finance_id);
    finances
}
```

**关于 `public` 标记**：View 声明中的 `public` 仅表示「允许客户端订阅该 View」，并不意味着数据公开——权限过滤在 View 函数体内部依据调用者身份执行。这与 Table 的 `public`（允许订阅整表、无任何过滤）含义截然不同。本项目 117 个 View 全部为 `public`，而 127 张 Table 全部为私有，二者配合构成读取面的权限边界（见 §2.3）。

**本项目的 View 编写规范**：所有 View 遵循统一的四步骨架——**获取权限范围 → 索引查找 → 谓词过滤 → 稳定排序**，并统一以 `my_` 为名称前缀，以表明「结果集从当前调用者的视角计算」这一语义。具体规范见 §2.3。

### 1.6 Procedure（过程）

**定义**：Procedure 是以请求-响应方式调用的服务端函数。与 Reducer 不同，Procedure 允许执行外部网络请求（如调用短信服务商接口），且可以向调用方返回结构化结果；相应地，它不以确定性事务的方式执行，不作为常规的数据写入入口。

Procedure 承担两类职责：

1. **外部输入输出**：Reducer 因确定性约束不能执行网络请求，此类操作（短信发送、验证码校验）由 Procedure 承担；
2. **大数据量的按需查询**：账单、财务流水、工资等历史数据量大，若全部纳入常驻订阅，将显著增加登录耗时、客户端内存占用与缓存刷新开销。此类数据由 Procedure 分页查询，客户端按需请求、一次性接收结果。

Procedure 与 Reducer 的对比：

| 对比项 | Reducer | Procedure |
| --- | --- | --- |
| 调用语义 | 提交一个状态变更命令 | 发起一次请求并等待响应 |
| 事务性 | 严格：一次调用一个事务，失败全回滚 | 非事务性入口，不用于常规写入 |
| 确定性 | 强制要求 | 无要求，可执行网络请求 |
| 返回值 | 仅成功 / `Err(String)` | 结构化查询结果 |
| 数据到达客户端的方式 | 经订阅推送 | 直接作为响应返回 |
| 本项目的用途 | 全部业务写入（269 个） | 3 个分页查询、4 个短信操作 |

本项目全部 7 个 Procedure 的清单见 README 第四节。

**Procedure 的能力边界**：它只能同步发起出站请求并等待响应，不能监听入站请求（webhook）、不能维持长连接、不能保存跨请求状态。此类外部集成需求（支付回调、物联网设备接入等）的承载方式见 §2.10。

### 1.7 Identity（身份）与连接模型

**定义**：Identity 是用户的全局唯一标识符，在连接建立时由连接携带的 Identity Token 确定。服务端代码在任何 View、Reducer、Procedure 中均可通过 `ctx.sender()` 获取当前调用者的 Identity。

本项目的身份解析链条如下：

```text
Identity Token（连接携带）
  → ctx.sender()（服务端获取 Identity）
  → 用户会话表 → 当前用户 → 当前租户（customer）
  → 角色、权限码、园区授权
  → Principal（调用者是谁）与 ReadScope（调用者能读取什么）
```

**安全规则**：`ctx.sender()` 是服务端唯一信任的身份来源。客户端作为参数传入的任何标识符——用户 ID、角色 ID、园区 ID——一律视为普通数据而非身份凭据，不得作为授权判断的依据。此规则不允许任何例外。

### 1.8 订阅与数据同步模型

**定义**：订阅（Subscription）是客户端向服务端注册查询语句、并持续接收该查询结果集变更的机制。其生命周期分为三个阶段：

1. **注册**：客户端经 WebSocket 提交查询（如 `SELECT * FROM my_finances`）；
2. **初始快照**：服务端立即返回当前的完整结果集；
3. **增量推送**：此后每当某次 Reducer 事务的提交改变了该查询的结果集，服务端自动将增量（插入、更新、删除的行）推送至客户端。

在此模型下，客户端本地持有一份由服务端持续维护的**结果集缓存**，界面渲染始终基于该缓存。客户端不存在「刷新数据」的操作——写入成功后，订阅缓存随推送自动更新，界面随之变化。

由此产生一条重要的开发准则：**不要为「写入后读取最新值」设计轮询、回调或手动刷新机制**。Reducer 调用成功即意味着变更已提交，订阅推送必然到达。例如客户端调用 `create_salary` 成功后，其订阅的 `my_salaries` 结果集将自动包含新增行。

### 1.9 ReducerContext（归约器上下文）

**定义**：ReducerContext（形参惯用名 `ctx`）是 Reducer 执行时可访问的环境对象，是 Reducer 与外部世界交互的唯一通道。其主要成员：

| 成员 | 类型 / 形式 | 用途 |
| --- | --- | --- |
| `ctx.db` | 数据库句柄 | 访问各表：`ctx.db.表访问器().操作()` |
| `ctx.sender()` | `Identity` | 当前调用者身份（§1.7） |
| `ctx.timestamp` | `Timestamp` | 本次事务的时间戳，同一事务内恒定 |
| `ctx.rng()` / `ctx.random()` | 随机数接口 | 确定性随机源（可重放） |

常用数据库操作形式如下：

```rust
ctx.db.finance().insert(row);                          // 插入（自增列以 0 占位）
ctx.db.finance().finance_id().find(id);                // 按主键查找 → Option<Finance>
ctx.db.finance().finance_by_park().filter(park_id);    // 按索引过滤 → 迭代器
ctx.db.finance().finance_id().update(row);             // 按主键整行更新
ctx.db.finance().finance_id().delete(id);              // 按主键删除
```

View 使用与之类似的 `ViewContext`，仅提供只读能力，且不提供时间源。

### 1.10 事务模型

一次 Reducer 调用即一个事务，具备原子性与隔离性：调用内对任意多张表的任意多次操作，要么全部生效，要么（在返回 `Err` 或 panic 时）全部撤销。

该性质决定了一条设计规则：**若两张表必须同时变更才能保持业务一致，则相应写入必须置于同一个 Reducer 内**，而不得拆分为两个 Reducer 由客户端顺序调用——后者在第二次调用失败时将产生不一致的中间状态。本项目中「工资记录 + 工资-流水关联 + 财务流水」三表的联动写入即遵循此规则（见 §2.7）。

### 1.11 客户端绑定（Client Bindings）

**定义**：客户端绑定是由 SpacetimeDB CLI 依据模块定义自动生成的客户端代码，包含每张表的行类型、每个 Reducer 与 Procedure 的类型安全调用接口、每个 View 的订阅接口。

本项目的绑定生成至仓库根目录的 `src/spacetime_bindings/`，由 Dioxus 前端引用。**凡修改了 Table、View、Reducer、Procedure 或其输入输出类型，必须重新生成绑定**，否则前端代码与服务端接口不一致。生成命令及注意事项见 README 第九节。生成产物不得手工修改。

### 1.12 确定性约束

**定义**：确定性（Determinism）指函数在相同输入下必然产生相同输出、且不产生外部可观察副作用的性质。SpacetimeDB 要求 Reducer 严格满足确定性，因为事务日志重放与多副本一致性均依赖「同一事务在任何时间、任何副本上重新执行结果相同」这一前提。

具体禁止项及替代方案：

| 需求 | 禁止使用 | 应当使用 |
| --- | --- | --- |
| 获取当前时间 | `std::time::SystemTime` 等系统时钟 | `ctx.timestamp` |
| 生成随机数 | `rand::thread_rng` 等系统随机源 | `ctx.rng()` / `ctx.random()` |
| 网络请求（短信等） | 在 Reducer 内发起请求 | 移至 Procedure |
| 文件读写 | 任何文件系统访问 | 不适用（数据一律入表） |

对本项目而言，该约束具有积极意义：它以平台强制的方式保证了每个 Reducer 都是「数据库状态与参数到新状态」的确定函数，即引用透明性由运行时保障而非编码自律。这是第二章所述函数式架构得以成立的基础。

### 1.13 Schema 演进约束

Schema（表结构定义的总和）的变更能力是 SpacetimeDB 与传统数据库差异最大、且对日常开发影响最直接的一点。下表为在 SpacetimeDB 2.6.1 上**逐项实验验证**的迁移行为（实验方法：本地探针模块，建表并插入数据后实施变更，不携带 `--delete-data` 参数发布，核对数据存续；结论与官方 Automatic Migrations 文档相互印证）：

| 变更类型 | `spacetime publish` 行为 | 数据 | 依据 |
| --- | --- | --- | --- |
| 新增表 / Reducer / View / Procedure | 自动迁移 | 保留 | 生产环境与本机实测 |
| 对已有表新增或移除索引 | 自动迁移 | 保留 | 本机实测 |
| 表尾追加带 `#[default(...)]` 标注的列 | 自动迁移，存量行填入默认值 | 保留 | 本机实测 |
| 追加无默认值标注的列 | 拒绝发布（提示 `requires a default value annotation`）；仅 `--delete-data` 可强制通过 | 强制通过则全库清空 | 本机实测 |
| 修改、删除、重排已有列 | 拒绝发布；仅 `--delete-data` 可强制通过 | 强制通过则全库清空 | 类型修改为本机实测，其余为官方文档 |
| 删除表；为已有列新增 Unique / 主键约束 | 官方文档列为禁止 | — | 官方文档，未逐项实测 |

`#[default(...)]` 标注的使用规则（均为 2.6.1 实测结论）：

1. 整数字面量必须携带类型后缀，如 `#[default(0u64)]`。无后缀的字面量将按 32 位宽度序列化，发布时服务端校验失败；
2. `Option` 类型的列应写 `#[default(None::<T>)]`——`None` 必须以 turbofish 语法标注类型，否则无法通过类型推断；
3. 默认值表达式在 const 上下文求值，带析构器的类型不可用作默认值——非空 `String` 列因此无法追加（编译错误 E0493）。需要追加字符串列时，应使用 `Option<String>`。
4. **追加列必须置于结构体末尾。** 插在既有列之间会被判定为重排，发布时报 `Reordering table X requires a manual migration` 并中止。将新列排在语义相近的既有列旁边是常见诱因，其代价是无法发布。
5. **默认值对存量行往往不是正确值。** 追加列时存量行一律填入默认值，而默认值通常只是类型上的零值：追加 `tenant_id: u64` 时存量行得到 `0`，语义上等同于「不属于任何租户」，此后凡按租户筛选的界面都不会再出现这些行。此类列在追加时须同时评估是否需要一次性回填，回填只能经 Reducer 完成——`spacetime sql` 的 `UPDATE` 写不了 `Option` 列。

**此类错误的暴露时机是其主要危害。** 列序错误与类型后缀错误在 `cargo check`、`cargo test`、`spacetime build` 阶段均无任何提示，只在 `spacetime publish` 的 schema 比对环节报出。而 CI 的顺序是先发布前端、后发布模块，因此模块发布失败会使线上停留在「前端绑定含新列、模块无该列」的错配状态。

列序一项已由完备性测试 `带默认值的列后面不能再有普通列`（`src/tables/mod.rs`）在提交前拦截：扫描 `src/tables`，一旦某列带 `#[default(...)]`，其后不得再出现普通列。判据的成立依据是——新表一次定稿无须 `#[default]`，只有事后追加的列才标注它。

由此得出的规则（依据实测结果于 2026-08 修订；本文早期版本称「新增索引、新增列一律须清库」，经实验证伪并更正）：

1. **扩展已有表时，优先考虑表尾追加带默认值标注的列**（适用于整数、布尔、`Option<T>` 等类型）；需要非空字符串列、或所建模的关系本身为一对多时，采用关联表（范例：`salary_finance`，§2.7）。
2. **修改、删除既有列不可平滑迁移**，此类需求必须重新设计：追加新列、新建关联表，或经数据导出重建。
3. **索引可以随时增删**，无需在建表时穷尽预判。
4. **CI 变量 `YIZU_SPACETIMEDB_DELETE_DATA` 必须保持 `false`。** 该变量为 `true` 时推送主分支将清空生产数据库。

任何涉及表结构的改动，提交前必须回答两个问题：是否修改或删除了已存在的列；是否追加了无默认值标注的列。任一答案为是，改动方案必须重新设计。详细论述见 README 第七节。

---

## 第二章　总体架构

### 2.1 架构风格概述

本项目的架构可概括为：**平台强制的命令查询职责分离（CQRS），叠加函数式核心与命令式外壳的代码组织，按业务域垂直切分**。本章依次说明各组成部分的定义、实现方式与设计依据。

为便于已阅读过函数式领域建模文献（如 Scott Wlaschin, *Domain Modeling Made Functional*）的读者建立对应关系，下表给出文献术语到本仓库实现的映射，各条目在后续小节展开：

| 文献术语 | 本仓库中的实现 | 详见 |
| --- | --- | --- |
| CQRS（命令查询职责分离） | Reducer 为命令端，View 为查询端；表全私有使分离成为强制 | §2.3 |
| Bounded Context（限界上下文） | 七个业务域目录，`tables/`、`views/`、`reducers/` 三层同名对齐 | §2.4 |
| Shared Kernel（共享内核） | `src/access.rs` 与 `reducers/shared/`：身份、授权、园区哨兵、输入规范化 | §2.5 |
| Aggregate（聚合） | 无聚合对象；聚合以「同一 Reducer 事务内共同变更的表集合」的形式存在 | §2.7 |
| Domain Event（领域事件） | 无异步事件设施；跨域记账在同一事务内同步完成 | §2.8 |
| Smart Constructor（智能构造器） | `validated_*` 函数族：行的唯一合法构造途径 | §3.1 |
| Railway-Oriented Programming | `Result<T, String>` 与 `?` 运算符构成的短路式校验链 | §3.2 |
| Functional Core / Imperative Shell | `check_*`、`normalize_*` 等纯函数为核心，Reducer 函数体为外壳 | §2.2 |
| Make Illegal States Unrepresentable | 在平台类型系统限制处降级为测试期保证，不降级为文档约定 | §2.9 |

### 2.2 函数式核心与命令式外壳

**定义**：「函数式核心、命令式外壳」（Functional Core, Imperative Shell）是一种代码组织模式：将业务决策——校验、计算、状态判断——实现为**纯函数**（不产生副作用、不依赖外部状态的函数，精确定义见 §3.1），构成系统的核心；将副作用——本项目中即数据库读写——集中于薄薄的外壳层。核心层逻辑密集且完全可测试；外壳层仅做读取、委托与写入，薄至几乎无需测试。

以 `reducers/hr/employee/salary.rs` 为例，该模式在本项目中的标准形态如下：

```rust
// ── 外壳层：读取、委托、写入，不含业务判断 ──────────────────────
#[spacetimedb::reducer]
pub fn create_salary(ctx: &ReducerContext, input: SalaryInput) -> Result<(), String> {
    AdminContext::require(ctx)?;                                  // 副作用：读取权限表
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_salary(ctx, 0, customer_id, input)?;      // 决策：校验并构造行
    let row = ctx.db.employee_salary().insert(row);               // 副作用：写入
    sync_salary_finance(ctx, &row);                               // 副作用：同步财务流水
    Ok(())
}

// ── 核心层：纯函数，不接触 ctx，可直接进行单元测试 ──────────────
#[pure_function::pure]
fn check_issued_amount(issued: Option<bool>, amount_cents: Option<i64>) -> Result<(), String> {
    if issued.unwrap_or(false) && amount_cents.is_none() {
        return Err("已发放的工资必须填写金额".into());
    }
    Ok(())
}
```

核心层函数的测试位于同一文件底部，测试函数名以中文陈述业务规则（测试规范见 §3.6）：

```rust
#[test]
fn 已发放的工资必须有金额() {
    assert!(check_issued_amount(Some(true), None).is_err());
}
```

**分层判据**：一段逻辑归属核心层还是外壳层，判断标准仅一条——**该逻辑是否需要访问 `ctx`**。参数校验、金额计算、文案拼装、状态机判断均不需要，应当抽取为纯函数；表的读取与写入需要，保留在外壳层，且外壳层不得混入其他逻辑。该判据不再单靠人工执行：核心层函数一律标注 `#[pure_function::pure]`，由属性宏在编译期拒绝任何接触副作用入口的代码（见 §3.8）。

### 2.3 命令查询职责分离（CQRS）

**定义**：CQRS（Command Query Responsibility Segregation，命令查询职责分离）是一种将系统的写入路径（命令）与读取路径（查询）分离为两套模型的架构模式。写入模型按数据一致性的需要设计，读取模型按使用方的查询需要设计，两者之间由某种同步机制衔接。

在传统技术栈中实施 CQRS 需要自行搭建三个部分：命令模型、查询模型、以及将写入同步至查询侧的机制（通常为事件总线，并伴随最终一致性窗口）。在本项目中，三者均由平台提供：

- **命令模型**＝私有表＋Reducer。表结构按写入一致性设计：关联表、软删除标记、审计字段、租户归属列。
- **查询模型**＝View。每个 View 按「特定身份的调用者应当看到什么」设计，`my_` 前缀即此语义的体现。
- **同步机制**＝订阅推送。不存在事件总线，亦不存在最终一致性窗口：事务提交与订阅推送由平台衔接，客户端缓存与数据库状态之间不产生持久分歧。

**分离的强制性**：由于 127 张表全部私有，客户端在物理上不存在绕过 View 读取数据的途径；由于客户端无法执行写语句，亦不存在绕过 Reducer 写入的途径。读写分离在本项目中不是编码约定，而是平台层面的结构事实。「忘记为某张表设置权限过滤」这一类事故在此结构下不可能发生——不存在无过滤的读取面。

**View 编写规范**：全部 117 个 View 遵循统一的四步骨架，以 §1.5 所引 `my_finances` 为准：

1. 获取权限范围：`current_read_scope(ctx)`，获取失败即返回空集；
2. 索引查找：优先使用索引缩小候选集（如 `finance_by_customer().filter(...)`）；
3. 谓词过滤：软删除标记（`!is_deleted`）与园区权限（`scope.allows_park(...)`）；
4. 稳定排序：保证客户端渲染顺序确定。

另有两条禁止性规定：

- 不得编写「返回全部数据」的 View。新增 View 的首要设计问题是：使用 `ReadScope` 的哪个方法执行过滤；
- View 不得写入数据、不得维护跨请求状态。View 是数据库状态到客户端缓存的纯投影。

View 之间允许组合复用：`my_finance_images` 直接调用 `my_finances(ctx)` 以复用其权限过滤逻辑，而非重复实现。此种复用应当优先于复制过滤条件。

### 2.4 限界上下文与垂直切片

**定义**：限界上下文（Bounded Context）是领域驱动设计（Domain-Driven Design, DDD）中的概念，指一个业务模型的适用边界：边界之内术语含义一致、模型自洽；跨越边界的协作必须显式进行。

本项目以**垂直切片**实现限界上下文：代码不按技术角色水平分层归类（如全部数据模型归一处、全部服务归一处），而是按业务域垂直切分为七个目录，且 `tables/`、`views/`、`reducers/` 三层使用完全一致的目录命名：

```text
修改「工资」相关功能
  → tables/hr/employee/    （数据定义）
  → views/hr/              （读取模型）
  → reducers/hr/employee/  （写入逻辑）
```

七个业务域的职责与规模见 README 第三节。

**边界的性质**：需要明确指出，Rust 的模块系统中 `tables::*` 在 crate 内部全局可见，业务域之间不存在编译器强制的访问隔离。上下文边界依靠**纪律**维持，具体而言是以下可枚举的清单：目前全部跨域写入仅有三处，方向一致（业务事实域 → 财务域），处理方式一致（记账以插入实现，撤销以 `is_deleted` 软删除实现）：

| 业务事实 | 代码位置 | 产生的财务流水 |
| --- | --- | --- |
| 账单创建 | `reducers/finance/billing/amount.rs` | 类别「账单收入」，方向「收入」 |
| 报销审批通过 | `reducers/finance/approvals/reimbursement.rs` | 类别「其他费用」，方向「支出」 |
| 工资标记发放 | `reducers/hr/employee/salary.rs`（经 `salary_finance` 关联表） | 类别「工资支出」，方向「支出」 |

此外存在一类**治理型跨域操作**：园区注销时须处置全库 20 张关联表（§2.9 详述）。它不构成两个特定域之间的耦合，而是经 `ParkChild` 清单集中管理的全局规则。

**审查要求**：代码评审中发现新的跨域写入（即某域的 Reducer 调用 `ctx.db.其他域的表()` 执行写操作）时，应当依次确认：该写入是否确有必要；能否改为本域内的关联表；若确有必要，是否遵循了「插入记账、软删撤销」的既有处理方式，并已在上表补充登记。

### 2.5 共享内核

**定义**：共享内核（Shared Kernel）指多个限界上下文共同依赖的一小部分模型与代码。共享内核中的语义变更影响全部上下文，因此其范围应当尽可能小、内容应当高度稳定。

本项目的共享内核及各组成部分的职责：

| 位置 | 内容 | 职责 |
| --- | --- | --- |
| `src/access.rs` | `Principal`、`ReadScope`、`Duty`、权限码常量 | 权限语义的唯一实现。`allows_park`、`allows_optional_park`、`has_code`、`owns_or_has_duty` 等方法定义「谁能读取什么」 |
| `reducers/shared/access.rs` | `AdminContext::require`、`current_customer_id`、各 `require_*` 函数 | Reducer 侧门卫：身份或目标记录缺失时返回中文错误信息 |
| `reducers/shared/park_ref.rs` | 常量 `NO_PARK`（值为 0） | 园区外键哨兵值及其设计依据（模块文档为完整的决策记录，见 §2.9） |
| `reducers/shared/validation.rs` | `required_text`、`normalize_optional_text`、`limited_optional_text`、`normalize_status` | 输入规范化：去除首尾空白、空串归一为 `None`、长度校验、MySQL 历史状态值归一 |
| `reducers/shared/bootstrap.rs` | 管理员初始化 | 由 `lifecycle.rs` 的 `init` 钩子调用，完成首次发布后的账号引导 |
| `views/shared/identity.rs` | `current_user`、`current_center_user` | View 侧身份判定，实现完全委托 `crate::access` |

最后一行体现共享内核的核心价值：View 与 Reducer 对「调用者是谁、能读取什么」的判定出自同一份实现，读取侧与写入侧的权限口径在结构上不可能发生分歧。

**准入标准**：仅当某项能力被两个以上业务域需要、且各域要求的语义完全一致时，方可进入共享内核。恰好可被多处复用的一般性工具函数不属于共享内核，应当就近放置于使用方所在的域。

### 2.6 数据建模通则

各业务域的表定义遵循以下统一规则（其依据分别见括号内章节）：

1. **金额一律以整数「分」存储**，字段类型 `i64`，命名后缀 `_cents`。全部 127 张表中浮点字段数为零。依据：浮点运算的舍入误差不可用于货币计算。
2. **时间一律使用 `spacetimedb::Timestamp`**，不以字符串存储时间（仅存量遗留 3 个 `Option<String>` 字段）。
3. **园区外键一律为 `u64`，以常量 `NO_PARK`（0）表示「不属于任何园区」**，不使用 `Option<u64>`（依据见 §2.9）。
4. **业务对象普遍具备**：自增主键、租户归属列 `customer_id`、审计时间 `created_at` / `updated_at`、软删除标记 `is_deleted`。
5. **软删除**：业务对象的删除以置位 `is_deleted = true` 实现，数据不作物理删除；物理删除仅适用于纯关系行（图片关联、`salary_finance` 等连接表）。财务流水的撤销（冲销）即软删除。
6. **模块注册**：表定义文件的 `mod` 声明名不得与表的 `accessor` 同名，否则将遮蔽属性宏生成的访问器项导致编译错误。统一写法：`#[path = "salary.rs"] mod salary_table;` 继而 `pub use salary_table::*;`。

### 2.7 聚合与事务边界

**定义**：聚合（Aggregate）是 DDD 中的一致性单元——一组必须共同保持业务一致的对象，外部只能通过聚合根提供的入口修改其状态，一次修改构成一个事务。

本项目不存在聚合对象、仓储层等显式设施；**聚合以事务边界的形式存在**，即「在同一个 Reducer 事务内共同变更的表集合」。两个代表性实例：

- **工资聚合**＝`employee_salary`（工资记录）＋`salary_image`（附件关联）＋`salary_finance`（工资-流水关联）＋`finance`（财务流水）。`create_salary`、`update_salary`、`delete_salary` 及其携带附件的变体是仅有的修改入口，四张表的一致性在单一事务内维护：标记发放时记账、修改金额时同步、取消发放时冲销、删除记录时解除关联。
- **园区聚合**＝`park` 及全库 20 张携带 `park_id` 列的表。园区注销时按 `ParkChild` 清单（§2.9）统一处置：存在账务记录（合同、账单、流水、报销等）时拒绝注销；纯资产台账（厂房、楼层、水电表、图片关联、授权关系）随园区级联软删除。

**设计规则**：若开发中发现两张表必须同时变更方能保持正确，则二者属于同一聚合，相应写入必须合并至同一个 Reducer；禁止将其拆分为多个 Reducer 由客户端顺序调用（依据见 §1.10）。

### 2.8 领域事件的同步实现

**定义**：领域事件（Domain Event）是 DDD 中表示「业务上已发生的事实」的建模元素，典型实现为：事实发生方发布事件，关注方异步订阅并作出反应，两侧经消息设施解耦。

以工资发放为例，教科书式的实现为：人事上下文发布 `SalaryIssued` 事件，财务上下文订阅该事件并登记支出流水。该方案的代价是事件基础设施本身，以及发布与消费之间的最终一致性窗口。

本项目未采用异步事件，而是**在同一事务内同步完成跨域记账**（即 §2.4 所列三处跨域写入）。其依据为：异步事件解耦所解决的问题——两个服务、两个数据库、无法共享事务——在单模块架构中不存在；平台已提供跨表原子性，引入事件设施将以引入最终一致性窗口为代价换取此处不需要的解耦。

一般性原则：**不引入当前架构中无对应问题的间接层**。

### 2.9 类型驱动设计的边界与补偿策略

函数式领域建模提倡「使非法状态不可表示」（Make Illegal States Unrepresentable）：借助类型系统使错误在编译期即不可能发生。典型手段包括以新类型（newtype）包装标识符（如 `ParkId(u64)`，防止将厂房 ID 误传给园区参数），以及用 `Option<T>` 表达可缺省值。

**平台限制**：SpacetimeDB 规定索引过滤参数必须实现 `FilterableValue` trait。该 trait 为密封（sealed）trait，仅覆盖整数、`bool`、字符串、`Identity`、`Timestamp` 与无载荷枚举。**newtype 与 `Option<u64>` 均不在其中**：以此类类型声明的列即使声明了 B 树索引，也无法调用 `.filter()` 执行索引查找，只能全表遍历。因此上述两种手段在表定义中不可用。

**补偿策略**：对因平台限制而无法获得编译期保证的约束，本项目遵循一条明确的降级路线：

> **编译期保证降级为测试期保证；不降级为文档约定。**

即：凡是希望编译器强制、而编译器无法强制的完整性约束，必须由一条会失败的自动化测试承担，而不是写入文档寄望开发者自觉遵守。

**完整案例——园区注销**（`reducers/rental/park/deletion.rs`）。业务约束为：每一张携带 `park_id` 列的表，在园区注销时都必须有明确的处置方式（拦截或级联），不得遗漏——遗漏即产生指向已删园区的孤儿数据。该约束由两道机制先后承担：

第一道，编译期。每张携带 `park_id` 的表对应枚举 `ParkChild` 的一个变体，处置方式定义于不含通配分支的 `match` 表达式。新增变体后，`spec`、`blocked_rows`、`cascade` 三个 `match` 同时产生「未覆盖变体」编译错误，处置方式的遗漏在编译期即被阻止：

```rust
pub(crate) enum ParkChild {
    RentalTenant, AmountBill, Finance, /* ……共 21 个变体 …… */
}

const fn spec(self) -> (&'static str, Disposition) {
    match self {
        Self::RentalTenant => ("rental_tenant", Disposition::Block("份合同")),
        Self::Finance      => ("finance",       Disposition::Block("笔财务流水")),
        Self::Factory      => ("factory",       Disposition::Cascade),
        // 注意：不存在 `_` 通配分支。遗漏变体是编译错误，而非运行时缺陷。
        /* …… */
    }
}
```

第二道，测试期。「表已建立但从未在 `ParkChild` 中登记」是编译器无法察觉的（不存在未覆盖的变体）。测试 `所有挂园区的表都已登记` 递归扫描 `src/tables` 目录的源代码，提取全部携带 `park_id` 列的表访问器名，与 `PARK_CHILDREN` 清单双向核对——存在未登记的表、或清单中存在已不携带该列的表，测试均告失败：

```rust
#[test]
fn 所有挂园区的表都已登记() {
    let declared = PARK_CHILDREN.iter().map(|c| c.spec().0.to_string()).collect::<BTreeSet<_>>();
    let actual = tables_with_park_column();   // 递归扫描 src/tables/**/*.rs
    assert!(actual.difference(&declared).count() == 0,
        "这些表带 park_id 但没在 ParkChild 里登记，园区注销时会留下孤儿数据");
    assert!(declared.difference(&actual).count() == 0,
        "这些表已经不带 park_id 了，请从 ParkChild 移除");
}
```

**同一策略对哨兵值的应用**：由于 `Option<u64>` 不可用作可过滤列，「不属于任何园区」以哨兵常量 `NO_PARK`（0）表达。哨兵值的固有风险在于语义散落——若各处代码各自判断 0 的含义，遗漏与分歧不可避免。本项目的处理是将哨兵语义**收拢至单一位置**：`ReadScope::allows_optional_park` 统一规定「`park_id` 为 `NO_PARK` 的行仅对不受园区限制的账号可见」，全部 View 经由该方法判断；同时 `park_ref.rs` 的模块文档完整记载了该设计的成因、代价与对策，作为后续维护者的决策依据。

### 2.10 外部集成与网关架构

**问题界定**：SpacetimeDB 的「单体」主张（§1.1）覆盖的范围是数据库进程内部——不再需要 ORM、连接池与缓存层；它不覆盖与外部系统的集成。WASM 沙箱在平台层面无法承担以下职责：监听入站回调（支付、合同签署等 webhook）、维持长连接（门禁与水电表设备的 MQTT 等）、维护跨请求的限流状态；Procedure（§1.6）只能同步发起出站请求并等待响应。这些是平台硬约束，无法以设计绕过。因此「完全单进程」的理想不可实现；应当保持的单体是**语言与工具链层面的单体**——全部服务端能力以 Rust 实现，不引入 Go、Java 等异构网关技术栈，集成需求的增长不增加团队的语言心智负担。

**既有设施**：本项目前端以 Dioxus fullstack 形态部署，其底层构建于 Axum 之上——`dioxus::launch` 默认初始化一个 Axum 服务器，`dioxus::serve` 允许取得底层 router 并自由挂载路由与中间件。亦即：系统中已经存在一个常驻的 Rust HTTP 服务进程，网关能力应当生长于其上，而非另立技术栈。

**架构决策**：外部集成能力由**现有的 Dioxus fullstack 进程承担**。webhook 接收、设备接入、限流、对账等网关职责，以「向该进程的 Axum router 挂载路由、向其 tokio runtime 派生后台任务」的方式实现；系统的部署组件保持**两个**——SpacetimeDB 的 `.wasm` 模块与 Dioxus/Axum 二进制。不新建独立网关服务，不另开编译单元，以简单性为先。落地形态如下：

```rust
// main.rs（server feature 下）
dioxus::serve(|| async move {
    let mut router = dioxus::server::router(app);

    // 网关自有路由——与 Dioxus 页面路由隔离的命名空间
    router = router
        .nest("/gateway", gateway::router())   // webhook 接收、健康检查
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // 启动时派生后台任务，订阅 SpacetimeDB 的集成任务表
    tokio::spawn(gateway::worker::run());

    Ok(router)
});
```

**预留的拆分路径（预案，不属于当前方案）**：若将来物联网设备的 MQTT 连接量或回调流量显著增大，与页面渲染争用同一 tokio runtime 线程池，可在同一 Cargo workspace 内把网关模块迁入独立二进制（如 `server/gateway/`），获得独立的进程与部署生命周期。由于语言与依赖自始共享，届时的拆分成本仅为把相关 `mod` 迁入新的二进制入口，业务代码基本不变。该预案的存在正是两组件方案可以放心采用的原因：当前的选择不会构成将来难以离开的结构。

**出站异步集成的标准模式：事务性发件箱（Transactional Outbox，规划）**

**定义**：事务性发件箱是分布式系统中解决「双写问题」——业务状态与对外通知必须同时成功或同时失败——的经典模式：把「需要通知外部」这一事实作为一行数据，与业务状态在**同一个事务**内写入；投递方异步读取该表执行实际调用，并写回执行结果。

该模式在本项目中的形态：

```text
业务 Reducer ──（同一事务：业务状态 + 任务行）──→ integration_task 表
integration_task（status = 'pending'）──（订阅推送，§1.8）──→ 网关后台任务
网关 ──（调用外部系统：短信 / 账单推送 / 硬件指令 / 合同签署）──→ 外部服务
网关 ──（调用专用 Reducer，幂等写回）──→ integration_task 状态更新
```

该模式的三项价值：

1. **解决双写问题**：业务变更与任务行在同一事务内落库，要么都成立、要么都不成立；Reducer 保持确定性（§1.12），外部调用完全移出事务；
2. **重试与审计免费获得**：任务行天然携带状态、尝试次数与时间戳——是否送达、失败几次、卡在何处，查表即知，失败任务可重新驱动；
3. **与 CQRS 结构一致**：任务表是一张普通私有表，写入走 Reducer、读取走订阅，不引入任何新概念。

**获取任务用订阅，不用轮询与消息队列**：网关以 SpacetimeDB Rust **Client SDK**（区别于模块侧的 `spacetimedb` 库）作为特权客户端接入数据库，订阅 `integration_task` 表中 `status = 'pending'` 的行，新任务经平台原生推送到达，零轮询延迟。引入 Kafka 等消息队列相当于重新搭建 §1.1 中平台已消除的设施，不予采用；轮询仅作为订阅断线重连后的补扫兜底。

**适用范围**：该模式覆盖「出站且允许最终一致性」的集成——批量催收短信、账单推送、硬件指令下发、电子合同签署等。它不覆盖三类需求：入站回调（webhook 由外部主动发起，仍由 Axum 路由接收）、用户等待结果的同步调用、MQTT 长连接。亦即：**任务表是网关的出站异步标准模式，而非网关本身**。现有登录短信验证码属用户等待的同步流程，维持 Procedure 现状（§1.6），不迁入该模式；批量催收短信为该模式的首个改造候选。

**任务表的设计要求**（依 §1.13，列须一次设计到位）：

1. **状态机**：`pending → processing → done / failed`，状态流转仅经专用 Reducer 完成；
2. **重试控制**：尝试次数与下次重试时间列，按退避策略重试；
3. **幂等键**：外部接口可能重复回执，写回 Reducer 必须依幂等键去重；
4. **死信处理**：重试次数耗尽转入 `failed` 并在管理界面可见，不允许静默丢失；
5. **租户归属**：`customer_id` 列，与全库建模通则一致（§2.6）。

**实现选型**：

| 需求 | crate | 说明 |
| --- | --- | --- |
| HTTP 服务框架 | `axum` | Dioxus 底层已在使用，webhook 接收路由直接复用 |
| 出站 HTTP 调用 | `reqwest` | 调用支付、合同、政务等外部接口 |
| 连接 SpacetimeDB | SpacetimeDB Rust Client SDK | 特权客户端：订阅任务表、回调专用 Reducer |
| MQTT（门禁 / 水电表） | `rumqttc` | 纯 Rust MQTT 客户端，与 EMQX 配合成熟 |
| 限流 | `governor` | 纯 Rust 令牌桶限流器，异步友好 |
| 重试 / 退避 | `backoff` 或 `tokio-retry` | 处理外部接口偶发失败 |
| 定时对账任务 | `tokio-cron-scheduler` 或 `tokio::time::interval` | 支付、合同对账补偿 |
| 对象存储（合同 PDF 等） | `aws-sdk-s3`（S3 兼容端点）或 `oss-rust-sdk` | 阿里云 OSS 建议走 S3 兼容模式 |
| 中间件（日志 / 追踪 / CORS） | `tower` / `tower-http` | 与 axum 无缝集成 |

**生态短板的如实说明**：熔断器（circuit breaker）在 Rust 生态中缺少与 Java Resilience4j 同等成熟度的主流实现（`failsafe-rs` 存在但维护活跃度一般）。该能力需自行实现一个三态状态机（closed / open / half-open，附失败计数），代码量在数十行内，实现成本低，但没有拿来即用的现成方案，须自行实现并测试。

**结论**：网关不是「再选一个技术栈」的问题，而是「向现有 Axum router 添加路由与后台任务」的问题。本项目以两个组件为定案：`.wasm` 模块承担业务状态与规则，Dioxus/Axum 进程承担页面服务与外部集成；拆分独立二进制仅作为流量隔离需求实际出现时的预案。

---

## 第三章　编程规范

本章规定编写服务端代码时应当遵循的具体规范。各条规范均以仓库中的现有代码为范例。

### 3.1 纯函数优先与智能构造器

**定义**：纯函数（Pure Function）指同时满足以下两个条件的函数：（一）输出完全由输入决定，相同输入必然产生相同输出；（二）不产生副作用，即不修改任何外部状态、不执行输入输出操作。纯函数可以脱离运行环境独立测试与推理。

本项目中，「不接触 `ctx`」的函数即纯函数（`ctx` 是副作用的唯一来源，见 §1.9）。每个 Reducer 源文件应当呈现如下标准结构：

```text
若干 #[spacetimedb::reducer] 外壳函数（薄，负责读取、委托、写入）
    ↓ 调用
validated_x(ctx, id, customer_id, input) -> Result<Row, String>   ← 智能构造器
    ↓ 调用
若干 check_* / normalize_* 纯函数（厚，承载全部业务规则，一律标注 #[pure_function::pure]）
    ↓
#[cfg(test)] mod tests（针对纯函数的单元测试）
```

**定义**：智能构造器（Smart Constructor）指「校验输入并构造合法值」的函数——调用方无法绕过校验直接获得该类型的值，从而保证凡存在的值皆合法。本项目的 `validated_*` 函数族即此模式：`create` 与 `update` 两条路径均从 `validated_x` 获取行值，业务规则仅实现一次。该函数返回 `Result<Row, String>`：成功时给出合法的行，失败时给出可直接展示给用户的中文错误信息。

（`validated_*` 因需查询关联记录存在性而接受 `ctx` 参数，属外壳层；其内部调用的 `check_*` 等规则函数必须保持纯函数性质。）

### 3.2 错误处理

**定义**：「错误即值」指以普通返回值（Rust 中的 `Result` 类型）表达失败，而非异常机制。「面向铁路的编程」（Railway-Oriented Programming）是对 `Result` 链式处理的比喻：程序沿成功轨道执行，任一环节失败即切换至错误轨道直达出口，后续环节不再执行。Rust 的 `?` 运算符即该模式的语言级支持。

规范如下：

1. **错误类型统一为 `String`**，Reducer 返回 `Result<(), String>`，校验函数返回 `Result<T, String>`，以 `?` 逐层传播。
2. **错误文案以最终用户为读者**，使用中文，直接说明业务原因：「已发放的工资必须填写金额」「当前用户未选择租户」。不得使用面向开发者的表述（`invalid input`）或错误码（`E1042`）。前端接收 `Err` 后直接展示，不存在翻译层。
3. **拦截类错误必须指明出路**。仅告知「不允许」而不说明「应当怎么做」的错误信息不合格。范例（园区注销被拦截时，`deletion.rs`）：文案逐项列出阻止注销的记录数量，并明确指出替代操作——「如果只是不再经营，请把园区状态改为『停用』；确实要删，请先处理这些记录。」

### 3.3 不可变数据与更新模式

本项目中行是值（value）：不存在 setter 方法、部分字段更新语句或共享可变状态。更新的唯一形式是**取出—修改本地副本—整行写回**：

```rust
let Some(mut finance) = ctx.db.finance().finance_id().find(link.finance_id) else { return };
finance.is_deleted = true;                      // 修改的是本地副本
finance.updated_at = Some(ctx.timestamp);
ctx.db.finance().finance_id().update(finance);  // 整行写回
```

软删除（§2.6 第 5 条）是同一理念对删除操作的延伸：业务对象的消亡同样是一次状态变更（`is_deleted` 置位），而非数据的消失。

### 3.4 迭代器管道

读取路径统一采用迭代器管道的声明式风格：`索引访问器().filter(键)` → `.filter(谓词)` → 终结操作（`.collect()`、`.count()`、`.next()`、`.any()`）。两条规则：

1. **索引查找先于谓词过滤**。`finance_by_customer().filter(id)` 是索引查找，复杂度与结果集成正比；`.filter(|f| !f.is_deleted)` 是逐行谓词判断。顺序颠倒即全表遍历。
2. **按需物化**。需要排序或多次遍历时方才 `.collect::<Vec<_>>()`；单值判断使用 `.next()`、`.any()` 等惰性终结操作，不将迭代器习惯性收集为向量。

### 3.5 穷尽模式匹配

**规范**：对枚举与状态组合的分支处理必须逐一列出全部情形，**禁止使用 `_` 通配分支**。依据：通配分支使编译器丧失完备性检查能力——新增枚举变体或新状态组合后，遗漏的处理逻辑不再表现为编译错误，而是表现为运行时的静默缺陷。

范例一：`sync_salary_finance`（`salary.rs`）将「发放状态 × 关联存在性」的四种组合完整列出，每种组合的处理一目了然：

```rust
match (salary.issued.unwrap_or(false), link) {
    (true,  None)       => { /* 首次发放：插入流水并建立关联 */ }
    (true,  Some(link)) => { /* 再次发放或修改金额：复用同一笔流水，更新并恢复 */ }
    (false, Some(link)) => { /* 取消发放：软删除流水，保留关联以备恢复 */ }
    (false, None)       => { /* 未发放且未记账：无操作 */ }
}
```

范例二：`deletion.rs` 的 `blocked_rows` 对不参与拦截的变体逐一列出而非以 `_` 概括，其代码注释说明了理由——若写通配分支，日后新增一张应当拦截的表而遗漏计数实现时，函数将静默返回 0 而放行注销，产生孤儿数据。

### 3.6 测试规范

服务端现有 97 个单元测试，全部作用于纯函数。**不对 `ctx` 做模拟（mock）**：依据 §2.2 的分层判据，需要 `ctx` 的逻辑本就不应承载业务规则，可测试性问题应通过调整分层解决，而非引入模拟设施。

测试分为两类：

1. **例证测试**：测试函数名以中文完整陈述一条业务规则，函数体给出正反例证。现有范例：`已发放的工资必须有金额`、`必填文本会去除首尾空格`、`状态值兼容_mysql_历史数据`、`只列出真正存在的记录并指向停用`。测试列表整体构成业务规格说明。
2. **完备性核对测试**：扫描源代码或核对清单，承接编译器无法承担的完整性约束（§2.9 的降级路线）。现有范例：`所有挂园区的表都已登记`、`所有名为_check_或_normalize_的函数都已标记为纯函数`（§3.8）。

**覆盖要求**：每个 `check_*` / `normalize_*` 纯函数至少具备一个正向用例与一个反向用例；涉及全局清单（如 `ParkChild`）的改动须确认相应核对测试仍然通过。

### 3.7 文档规范

模块级文档注释（`//!`）用于记载**设计决策及其依据**——当时面临的约束、被否决的备选方案、选定方案的代价与对策。行内注释仅说明代码自身无法表达的约束，不复述代码行为。

现有范例：`park_ref.rs`（为何不使用 `Option<u64>`）、`deletion.rs`（为何采用清单而非逐表硬编码、处置方式的选取标准）、`salary_finance.rs`（为何新建关联表而非在工资表增加列）。

衡量标准：当后续维护者试图「简化」某个看似迂回的设计时，模块文档应当足以说明该设计存在的原因，使其在充分知情后再做决定。

---

### 3.8 纯函数标记宏 `#[pure]`

**定义**：`#[pure_function::pure]` 是本仓库自制的属性宏（Attribute Macro，一种在编译期检查或改写被标注代码的 Rust 元编程机制），实现于 workspace 成员 `pure/`（仓库根）（crate 名 `pure_function`）。将其标注在函数上，编译器即检查该函数的签名与函数体中是否出现「副作用入口」标识符，一经出现，就在违规标识符的精确位置产生编译错误。§2.2 的分层判据由此从人工纪律升级为编译期保证。

**原理**。本系统两端的副作用都采用能力传递风格（Capability-Passing Style）：副作用能力只能经参数或调用约定获得，不存在全局句柄。

- **服务端（SpacetimeDB 模块）**：副作用只有一扇门 `ctx`。数据库、时钟、随机数均须经由 `ReducerContext` / `ViewContext` 获得（§1.9、§1.12）。
- **前端（Dioxus）**：副作用的门是 Hook（`use_*` 系列函数）、响应式状态 `Signal`、异步任务 `spawn` 与浏览器 API（`web_sys` / `js_sys`）。页面 `model.rs` 层的函数不接触这些门，才能脱离组件树独立测试。

因此「签名与函数体不出现这些门」在本代码库内几乎等价于纯函数。宏对被标注函数的整个 token 流做递归扫描，嵌套在 `format!` 等宏调用内部的标识符同样会被扫到。

**禁用清单**（按类别归纳；完整清单与各项理由见 `pure/src/lib.rs`）：

| 类别 | 标识符 | 适用端 |
| --- | --- | --- |
| 上下文 | `ctx`、`ReducerContext`、`ViewContext`、`AnonymousViewContext`、`DbContext`、`LocalReadOnly` | 服务端 |
| 平台 API | `spacetimedb` | 服务端 |
| 响应式状态 | `Signal`、`ReadOnlySignal`、`GlobalSignal`，以及一切以 `use_` 开头的标识符（Hook） | 前端 |
| 外壳操作 | `spawn`、`navigator`、`dioxus` | 前端 |
| 浏览器与网络 | `web_sys`、`js_sys`、`wasm_bindgen`、`gloo_net`、`reqwest`、`tokio` | 前端 |
| 全局状态 | `static`（`&'static` 生命周期不受影响）、`thread_local`、`unsafe` | 两端 |
| 可观测输出 | `log`、`println`、`eprintln`、`print`、`eprint`、`dbg` | 两端 |
| 时钟 | `SystemTime`、`Instant` | 两端 |

违规时的报错（真实输出，定位到违规标识符所在行列）：

```text
error: #[pure] 违规：`ctx`——ctx 是副作用的唯一入口，纯函数不能接触它
   --> server/spacetimedb/src/reducers/hr/employee/salary.rs:125:35
```

**标注范围与命名约定**。宏只检查被标注的函数；哪些函数必须标注，由命名约定加完备性核对测试闭环（与 §2.9 的降级路线同一思路）：

| 端 | 约定 | 兜底测试 |
| --- | --- | --- |
| 服务端 | 名为 `check_*` / `normalize_*` 的函数必须标注；需要 `ctx` 的检查函数改名 `require_*`，需要 `ctx` 的校验构造改名 `validated_*` | `所有名为_check_或_normalize_的函数都已标记为纯函数`（`reducers/shared/validation.rs`） |
| 前端 | 页面 `model.rs` 的公开函数必须标注；读时钟的函数以 `now_` / `today` 开头命名，豁免标注 | `model_rs_的公开函数都已标记为纯函数或按时钟命名`（`src/pages/mod.rs`） |

三道防线协同：命名约定（人读代码时可辨认）→ 完备性测试（漏标必失败）→ 宏检查（标了必纯）。

**局限（如实声明）**：（一）纯度不传递——宏不检查被调函数，被调纯函数依赖上述命名约定与测试覆盖；（二）经结构体字段走私 `ctx` 引用的病态写法拦不住（本仓库无此形态，靠评审）；（三）堆分配与 panic 不视为副作用，与本项目对「纯」的定义（可脱离环境单测、同输入同输出）一致。

## 第四章　开发流程

### 4.1 新增业务对象的标准步骤

以在某业务域新增业务对象 X 为例，标准流程共八步：

1. **定义数据表**（`tables/<域>/…/x.rs`）：自增主键；租户归属列 `customer_id`；审计时间 `created_at`、`updated_at`；业务对象增加软删除标记 `is_deleted`；声明所需索引（至少含按 `customer_id` 的索引；如关联园区，`park_id` 列类型为 `u64` 并按需建立索引）。依据 §1.13：索引可事后增删，但**列的修改与删除不可迁移，列设计必须一次定稿**。
2. **注册模块**（所在 `mod.rs`）：`#[path = "x.rs"] mod x_table;` 与 `pub use x_table::*;`。`mod` 名不得与表访问器同名（§2.6 第 6 条）。
3. **登记园区关联**：若表携带 `park_id` 列，在 `reducers/rental/park/deletion.rs` 的 `ParkChild` 枚举中新增变体，并依编译错误补齐三处 `match`。若遗漏此步，测试 `所有挂园区的表都已登记` 将失败并指明缺失的表名。
4. **实现校验核心**：智能构造器 `validated_x(...) -> Result<X, String>` 及其调用的 `check_*` 纯函数。业务规则仅在此实现一次。
5. **实现 Reducer 外壳**（`reducers/<域>/…`）：门卫（`AdminContext::require` 或相应 `require_*`）→ 校验 → 写入。须与 X 同时变更的关联行写入同一 Reducer（§2.7）。
6. **实现 View**（`views/<域>/…`）：`my_xs`，遵循四步骨架——获取 `current_read_scope` → 索引查找 → 谓词过滤（`is_deleted` 与 `allows_park`）→ 稳定排序（§2.3）。
7. **测试与构建**：为第 4 步的纯函数编写中文命名的正反用例；执行 `cargo test --manifest-path server/spacetimedb/Cargo.toml` 与 `spacetime build --module-path server/spacetimedb`。
8. **生成绑定**：执行 `spacetime generate`（命令见 README 第九节），确认生成产物的变更仅为新增或修改对应类型。

### 4.2 提交前检查清单

提交代码前逐项确认：

- [ ] 业务规则是否全部位于纯函数中，且均有中文命名的测试用例；
- [ ] 是否引入了新的跨域写入；若是，是否满足 §2.4 的审查要求并已登记；
- [ ] 错误文案是否面向最终用户、是否为拦截类错误指明了出路（§3.2）；
- [ ] 是否出现了 `_` 通配分支（§3.5）；
- [ ] 模块文档是否记载了本次的非显然设计决策（§3.7）。

### 4.3 发布安全评估

推送主分支即触发生产部署（流程见 README 第十节），因此每次提交前必须完成 Schema 影响评估：

1. 本次改动对 Schema 的影响是否属于可平滑迁移的类别（新表、新 Reducer / View / Procedure、索引增删、带默认值标注的追加列，见 §1.13）；
2. 是否修改或删除了已存在表的列、或追加了无默认值标注的列——若是，依据 §1.13 该改动无法平滑发布，方案必须重新设计；
3. CI 变量 `YIZU_SPACETIMEDB_DELETE_DATA` 是否保持 `false`（确认命令：`gh variable list | grep DELETE_DATA`）；
4. 部署完成后按 README 第十节执行核对：CI 状态、模块日志、生产数据抽查、前端可用性。
