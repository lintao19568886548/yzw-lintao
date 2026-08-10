# 云园慧控 · Parkwise

工业园区运营管理系统。租赁、财务、人事、设备、门禁、维护六条业务线在一套系统里闭环，面向园区管理方自用，按客户私有化交付。

技术上它有一个不常见的形态：**没有独立的应用服务器层**。业务数据、事务逻辑、权限过滤和实时推送全部在一个跑在 SpacetimeDB 里的 WASM 模块内完成，浏览器通过 WebSocket 直连数据库。前端是 Dioxus 0.7 编译出的 WASM。整个仓库只有 Rust。

> 后端的设计原理与编程规范见 [server/ARCHITECTURE.md](server/ARCHITECTURE.md)（四章，面向合作开发者），后端的「有什么、怎么跑」见 [server/README.md](server/README.md)。本文是仓库层面的入口：三个 crate 如何拼在一起、前端怎么跑、怎么测、怎么发。

规模（统计截至 2026-08）：

| 项目 | 数值 |
| --- | --- |
| Rust 代码 | 约 13.5 万行（前端 + 服务端 + 宏） |
| Table / View / Reducer | 127 / 117 / 269 |
| 前端页面文件 | 103 个 |
| 单元与渲染测试 | 前端 183 项、服务端 120 项 |
| 纯函数标记 | 192 处 `#[pure_function::pure]` |

---

## 一、业务图景

系统的使用者是园区管理方，**不是租客**：

```
园区老板  →  园区经理  →  厂房 / 宿舍楼层  →  租客
（多园区）   （日常运营）   （出租单元）        （不登录系统）
```

租客不持有账号。所有与租客有关的动作——催交、收款、抄表、开门、报修——都由园区一侧的人在系统里记录。这个前提决定了很多设计：例如租金收缴的闭环靠「园区经理点确认」推进，而不是靠租客端的状态回传（见 [docs/租金收缴与对账确认流程.md](docs/租金收缴与对账确认流程.md)）。

功能模块与侧边栏一一对应：

| 分组 | 页面 |
| --- | --- |
| 工作台 | 运营总览、数据地图 |
| 设备管理 | 设备总览、智能水电表管理、摄像头管理、门禁设备管理、边缘计算设备 |
| 租赁 | 租赁总览、园区管理、待租厂房、租户管理、合同管理 |
| 人事 | 人事总览、人事管理、工资管理、角色管理 |
| 财务 | 财务总览、财务管理、账单管理、账期结转、报销管理、报销审核 |
| 门禁管理 | 车辆出入管理、访客管理、访客登记 |
| 维护管理 | 消防管理、变压器管理、电梯管理、报修工单 |

页面按权限显示。权限模型（角色、菜单、园区授权范围三者的关系）见 [docs/权限模型设计.md](docs/权限模型设计.md)。

## 二、两个服务端，各管一摊

这是本仓库最容易被误解的地方：`server` 这个词在项目里指两样不同的东西。

**其一，SpacetimeDB 模块**（`server/spacetimedb/`）——业务系统的真正后端。所有业务表、所有写入事务、所有权限过滤都在这里。浏览器订阅 View 拿数据，调用 Reducer 写数据，中间没有 REST 层。短信登录也在这里，走 Procedure。

**其二，Dioxus 全栈服务端**（`src/services/` 里带 `#[post("/api/…")]` 的函数）——只承担一件事：**替浏览器保管密钥并代为访问外部服务**。目前 8 个端点：

| 端点 | 用途 | 密钥来源 |
| --- | --- | --- |
| `/api/storage/r2/*` | Cloudflare R2 图片上传与删除 | `R2_*` |
| `/api/billing/excel/*` | 账单 Excel 导入解析与导出 | 无（纯计算） |
| `/api/billing/excel/analyze`、`/api/contract/images/analyze` | 阿里云百炼识别账单表格与合同图片 | `ALIYUN_BAILIAN_KEY` |
| `/api/smart-meter/*` | 抄表厂商（YMSINO、合众）HTTP 接口 | `YIZU_YMSINO_*`、`YIZU_HEZHONG_*` |

判据很简单：**需要密钥、需要解析二进制、或者对方是第三方 HTTP 服务的，走 Dioxus 服务端；其余一律走 SpacetimeDB 模块**。智能水电表因此没有任何 SpacetimeDB 表，数据实时来自厂商平台。

## 三、仓库结构

```text
parkwise/
├─ src/                     Dioxus 前端（Web/WASM）+ 全栈服务端函数
│  ├─ pages/                业务页面，每个目录含 model.rs（纯逻辑）与视图组件
│  ├─ components/           通用组件与布局（含 layout/sidebar.rs 侧边栏）
│  ├─ services/             SpacetimeDB 连接、订阅与写入封装；外部集成
│  ├─ permissions/          路由权限判定
│  ├─ state/                全局响应式状态与数据域
│  ├─ spacetime_bindings/   spacetime generate 的产物，不手改
│  └─ router.rs             路由表
├─ server/
│  ├─ spacetimedb/          SpacetimeDB WASM 模块（crate: parkwise_server）
│  ├─ ARCHITECTURE.md       服务端架构说明书
│  └─ README.md             服务端使用与发布说明
├─ pure/                    #[pure] 过程宏：编译期拦截副作用（见第五节）
├─ tools/onvif-discover/    装机现场的摄像头发现工具，独立编译，不进主构建
├─ docs/                    业务与技术专题文档（见第七节）
├─ deploy/                  生产环境 nginx、systemd 与部署脚本
├─ scripts/dev/             本地开发辅助脚本
└─ .github/workflows/       推送 main 即发布的 CI/CD
```

工作区成员是 `.`、`pure`、`server/spacetimedb` 三个。`tools/onvif-discover` 被 `exclude` 掉——它装机时用一次，没必要让每次 `cargo test` 都编译它，也没必要让它的依赖进主程序的依赖树。

前端只保留 **Dioxus Fullstack Server 与 Web/WASM 客户端**，不构建 Desktop、Android、iOS 原生客户端。

## 四、本地开发

### 前置工具

```bash
# Rust 稳定版 + WASM 目标
rustup target add wasm32-unknown-unknown

# Dioxus CLI
cargo install dioxus-cli          # 或 cargo binstall dioxus-cli

# SpacetimeDB CLI，版本必须固定为 2.6.1
curl -sSf https://install.spacetimedb.com | sh
spacetime version install 2.6.1
spacetime version use 2.6.1
```

**2.6.1 这个版本号不是建议值。** 本地 CLI、CI 与生产服务器三者必须一致，生成客户端绑定尤其必须用 2.6.1 的 CLI——版本不一致产出的绑定与线上模块对不上，症状是运行期报找不到表或字段，而不是编译错误。

### 启动前端

```bash
dx serve --platform web
```

或用脚本（会先收掉上一次残留的 `dx serve` 再启动，适合 IDE 里反复点运行）：

```bash
scripts/dev/serve-web.sh          # 启动
scripts/dev/serve-web.sh stop     # 只停不启
```

### 环境变量

复制 `.env.example` 为 `.env` 并按需填写。密钥类变量只由 Dioxus 服务端读取，不会进入浏览器。

| 变量 | 说明 |
| --- | --- |
| `YIZU_PUBLIC_APP_ORIGIN` | 对外公开的站点来源，用于二维码、分享链接与页面自举 |
| `YIZU_SPACETIMEDB_URI` | 浏览器直连的 SpacetimeDB 来源，HTTPS 时 SDK 自动用 WSS |
| `YIZU_SPACETIMEDB_SERVER_URL` / `_DATABASE` | 服务端校验浏览器提交的身份令牌时使用 |
| `R2_*` | Cloudflare R2 对象存储 |
| `ALIYUN_BAILIAN_KEY` | 账单表格与合同图片的 AI 识别 |
| `YIZU_YMSINO_*` / `YIZU_HEZHONG_*` | 智能水电表厂商接口 |

### 手机浏览器联调

获客二维码等功能需要用手机实际访问。手机与电脑须处于同一局域网，且 SpacetimeDB 地址对手机可达。远程 SpacetimeDB 只有 Tailscale 地址时，先起局域网透明转发：

```bash
node scripts/dev/spacetimedb-lan-proxy.mjs
```

然后用局域网地址启动，**必须加 `--release`**：

```bash
YIZU_PUBLIC_APP_ORIGIN=http://192.168.3.45:8080 \
YIZU_SPACETIMEDB_URI=http://192.168.3.45:3000 \
  dx serve --release --addr 192.168.3.45 --port 8080 --open false
```

`--release` 不是为了跑得快，是为了**能打开**。本项目依赖很多（组件库、SpacetimeDB SDK、AWS SDK），调试构建的 WASM 带完整 debuginfo 会到几百 MB，手机浏览器编译这么大的模块直接卡死。

### 服务端开发

构建、测试、生成绑定的命令见 [server/README.md 第九节](server/README.md)。新增业务对象的标准八步流程见 [ARCHITECTURE.md §4.1](server/ARCHITECTURE.md)。

## 五、测试与工程约束

```bash
cargo test                                              # 前端 183 项
cargo test --manifest-path server/spacetimedb/Cargo.toml # 服务端 120 项
cargo check --target wasm32-unknown-unknown             # WASM 目标必须零错误
```

`cargo check` 的宿主目标过了不代表 WASM 过——浏览器没有 `SystemTime`，条件编译的分支只有换目标才会被真正编译。**两个目标都要过。**

测试分三类，各自防的是不同的事：

**其一，纯函数的正反用例。** 业务规则只在纯函数里实现一次，用中文命名的测试覆盖。规则一改，测试立刻红。

**其二，渲染测试。** 用 `dioxus_ssr::render_element` 在与真实应用相同的上下文里渲染页面，防的是「路由字符串对了但页面一进去就崩」。

**其三，完备性测试——本项目最值得注意的一类。** 它们不测某个函数的行为，而是**扫源码，检查「该登记的都登记了」**：

| 测试 | 防止的遗漏 |
| --- | --- |
| `所有挂园区的表都已登记` | 新表带了 `park_id` 却没进 `ParkChild`，删园区时留下孤儿数据 |
| `每张订阅的表都接了刷新回调` | 订阅了表却没接刷新，写入成功但界面不动 |
| `每个场景对应的刷新函数集合与预期一致` | 数据域切换时漏刷某张表 |
| `所有按租户自愈的初始化都已挂上` | 新租户缺默认菜单或权限码 |
| `所有名为 check_ 或 normalize_ 的函数都已标记为纯函数` | 业务规则写进了带副作用的函数里 |
| `每个动作都登记在动作清单里` | 状态机新增动作却没进解释器清单 |

这类测试是有来历的：设备台账曾经出现过「写入成功但列表不刷新」的线上问题，根因就是加了订阅和刷新函数，唯独漏了 `wire_table_refresh!` 那一处登记。三处要改的地方，人只会记住两处。**「漏了」这种错误靠自律防不住，只能靠让它编译不过或测试变红。**

`#[pure_function::pure]` 是同一思路的编译期版本：被标记的函数体里出现 `ctx`、`Signal`、`use_*`、`SystemTime`、`log` 等副作用入口，直接 `compile_error!`。原理与已知局限见 [pure/src/lib.rs](pure/src/lib.rs) 的模块文档。

## 六、发布

生产工作流已在 P0-01 改为仅允许手动触发；推送 `main` 不再触发生产部署。完整的受保护 Environment 审批、不可变 artifact 和发布前置门禁继续由 P0-16 实施。

因此每次提交前必须做 Schema 影响评估。SpacetimeDB 2.6.1 的迁移红线：

| 可平滑迁移 | 不可迁移 |
| --- | --- |
| 新增表、Reducer、View、Procedure | **修改已有表的列** |
| 增删索引 | **删除已有表的列** |
| 带 `#[default(...)]` 的**尾部追加**列 | **删除表** |
| | 追加列但没写默认值 |

**列的设计必须一次定稿。** 撞上不可迁移的改动时必须重新设计；生产发布代码中已永久移除清库和重建入口，不再依靠可误设的布尔变量保护数据。

发布后的核对清单（CI 状态、模块日志、生产数据抽查、前端可用性）见 [server/README.md 第十节](server/README.md)。

私有化交付方案（每客户一容器、随机初始密码 + 首登强制改密）见 [docs/交付与部署方案.md](docs/交付与部署方案.md)，生产环境的 nginx 与 systemd 配置见 [deploy/](deploy/)。

## 七、文档索引

**架构与规范**

- [server/ARCHITECTURE.md](server/ARCHITECTURE.md) —— 服务端架构说明书：SpacetimeDB 概念、总体架构、编程规范、开发流程
- [server/README.md](server/README.md) —— 服务端有什么、怎么跑、怎么发
- [docs/权限模型设计.md](docs/权限模型设计.md) —— 角色、菜单、园区授权范围
- [docs/抽象泄露评估.md](docs/抽象泄露评估.md) —— 哪些底层细节不得不暴露给上层，以及为什么

**业务专题**

- [docs/园区管理.md](docs/园区管理.md)
- [docs/待租厂房.md](docs/待租厂房.md)
- [docs/租金收缴与对账确认流程.md](docs/租金收缴与对账确认流程.md) —— 催交、对账确认与账期结转
- [docs/设备管理.md](docs/设备管理.md) —— 传感器与执行终端的台账模型
- [docs/智能水电表管理.md](docs/智能水电表管理.md)
- [docs/边缘计算设备与摄像头接入.md](docs/边缘计算设备与摄像头接入.md) —— Windows PC 作边缘计算设备直连数据库
- [docs/消防设施台账与扫码巡检.md](docs/消防设施台账与扫码巡检.md)
- [docs/变压器台账与扫码巡检.md](docs/变压器台账与扫码巡检.md)
- [docs/电梯台账与扫码巡检.md](docs/电梯台账与扫码巡检.md)
- [docs/AI经营参谋技术方案.md](docs/AI经营参谋技术方案.md)

**交付**

- [docs/交付与部署方案.md](docs/交付与部署方案.md)
- [deploy/README.md](deploy/README.md) —— 生产部署的 Secrets 与 Variables 配置

## 八、现场工具

[tools/onvif-discover/](tools/onvif-discover/) —— 装机现场用的 ONVIF 摄像头发现工具。WS-Discovery 组播扫描局域网内的摄像头，用 WS-Security UsernameToken 取回 RTSP 地址与设备信息。独立工作区，需单独进目录 `cargo run`。

---

## 附：短信登录

短信发送与验证码校验全部在 SpacetimeDB 模块内：

- `procedures/center/auth/sms.rs` —— 调用联麓短信，并在显式事务中校验验证码
- `tables/center/auth/sms.rs` —— 私有的供应商配置与一次性验证码摘要
- `sms/` —— 联麓签名协议与验证码 HMAC

前端通过生成的 `connection.procedures` 直接调用。验证码有效期 5 分钟，60 秒内不可重复发送，连续输错 5 次自动失效。**供应商密钥、验证码 Pepper 与验证码摘要都不进客户端订阅**——它们在私有表里，而私有表没有对外的读取面。
