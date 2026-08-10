# P0-01 破坏性操作盘点与生产永久禁用证据

> 任务：P0-01
> 分支：`phase0/p0-01-disable-destructive-ops`
> 审计方式：纯本地源码、脚本、工作流和部署配置静态审阅
> 边界：未连接或修改测试/生产服务器；未执行数据库、对象、容器卷或项目文件删除；未读取或记录任何生产配置值

## 1. 判定口径

| 等级 | 定义 | P0-01 处理 |
|---|---|---|
| D0 | 全库/全模块清空、重建、数据库删除、任意发布 shell、公开模块发布入口 | 生产代码路径永久移除或直接拒绝 |
| D1 | 生产发布和环境控制 | 仅手动触发、环境未知失败关闭、CI 失败不进入部署；完整审批与 artifact 门禁留给 P0-16 |
| D2 | 以一个业务根对象触发的事务级联、迁移内部批量更新 | 保留业务语义，记录授权和事务保护；后续补影响预览与统一审计 |
| D3 | 单资源业务删除、临时验证码/关系行清理 | 不删除正常能力；记录现有权限、幂等和审计缺口 |

对应用户要求的六类结论为：D0/D1 属于“生产可达的真实风险”，D2/D3 属于“普通单对象业务删除或受控事务级联”，开发代理脚本属于“开发环境专用”，单元测试状态清理属于“测试或模拟代码”，命令说明属于“文档示例”，字符串截断、集合清空和网络连接销毁属于“误报”。

## 2. 生产及高风险入口逐项证据

| ID | 文件路径 | 行为说明 | 触发入口 | 影响对象 | 修改前可能到达生产 | 风险 | 修改前保护 | 本次修改 | 修改后验证 | 剩余风险 |
|---|---|---|---|---|---|---|---|---|---|---|
| WF-01 | `.github/workflows/deploy-production.yml` | `main` 推送自动进入生产构建和部署 | push / 手动 workflow | Web、SpacetimeDB 模块、运行配置 | 是 | 严重 D1 | deploy job 依赖 build；声明 `production` Environment | 删除 push 触发，仅保留手动触发；在 build 首步加入 P0-01 扫描 | 静态检查必须只有 `workflow_dispatch`；负向样例加入 push 时扫描失败 | Environment 审批人、不可变 artifact 和 Secrets 前置门禁属于 P0-16，未提前实施 |
| WF-02 | `.github/workflows/deploy-production.yml` | 向远端传入清库变量、强制发布开关和任意发布命令 | 生产 deploy job | 整个 SpacetimeDB 模块数据 | 是 | 严重 D0 | 清库变量缺省关闭，但可被仓库配置误改 | 删除三类输入；只传固定生产环境标识和固定非破坏发布开关 | 扫描禁止遗留变量名、数据删除参数及动态 shell | 仍允许手动执行固定非破坏 publish；schema 不兼容时应失败，不会清库 |
| SH-01 | `deploy/scripts/remote_deploy.sh` | 接受任意命令并经 shell 执行；可把全库删除参数加入 publish | 远端部署脚本环境变量 | 整个 SpacetimeDB 模块数据 | 是 | 严重 D0 | 单布尔默认值与注释提醒 | 删除任意命令、清库和 force 分支；publish 参数固定且不含数据删除；生产检测到破坏性配置项名称直接拒绝；未知环境拒绝 | PowerShell 扫描、shell 静态检查、危险配置名负向测试、危险参数复核 | 脚本仍会重启服务并发布兼容模块，这是正常部署而非数据销毁；完整拆分属于 P0-16 |
| SH-02 | `deploy/scripts/remote_deploy.sh`、`.github/workflows/deploy-production.yml` | 使用可变目录递归清理部署工作区 | 远端部署 | 临时部署文件；错误变量可能扩大范围 | 是 | 高 D1 | 仅依赖默认临时路径 | 移除递归清理；每次使用 GitHub run 派生的全新临时目录，目录已存在即拒绝；脚本只接受限定临时路径 | 扫描禁止生产文件出现递归删除命令；脚本静态检查 | 远端临时目录保留策略尚未设计，但不影响业务数据安全；本任务不执行服务器清理 |
| NGX-01 | `deploy/nginx/yizu-furong.org.conf` | 公网代理允许 SpacetimeDB 模块发布 API 和数据库 DELETE 请求通过 | 公开 HTTPS 代理路径 | 模块 schema 和全部数据库数据 | 是，取决于运行时认证与网络 | 严重 D0 | 依赖 SpacetimeDB 身份；另一个监听端口是否被网络隔离无法从本地确认 | 在所有公开代理入口前统一匹配并返回拒绝；固定部署脚本只通过服务器本机目标发布 | 扫描要求数据库 DELETE、模块 publish 两类拒绝规则存在，且两个公开代理入口都引用该规则；Nginx 静态语法检查 | 本地只修改版本库配置，现有服务器运行配置未被访问或重新加载；生效部署归属 P0-16 |
| OBJ-01 | `src/services/storage/r2_cleanup.rs` | 图片元数据无引用后逐个调用对象存储物理删除，单次最多 50 个 | Dioxus Server Function | 工资凭证、合同及业务图片对象 | 是 | 严重 D0（不可恢复文件删除） | 管理员凭据、元数据无引用、对象 key 格式和数量上限 | 生产与未知环境永久拒绝物理删除；仅 `local/test`、独立非生产数据标识和精确确认信息三项同时满足才允许；生产工作流不传确认项 | Rust 纯函数负向测试覆盖 production、unknown、伪造确认、未隔离数据；编译和测试检查 | 30 天回收站、版本恢复、财务附件审批尚未实现，属于 P0-05；完成前生产只保留对象、不物理删除 |
| DB-01 | `server/spacetimedb/src/reducers/rental/park/deletion.rs` | 单个园区注销会软删除资产、硬删纯关系/图片元数据，并阻止带账和历史记录的删除 | `delete_park` Reducer | 园区及其资产/授权关系 | 是 | 高 D2 | 租赁管理员、园区访问权、完整子表清单、账务/历史 blocker、事务语义 | 保留正常业务能力；登记为 D2，不纳入 D0 禁令 | 复核授权调用、blocker 和软删/硬删分类；相关既有测试 | 尚无统一审批、影响预览和不可变审计，后续由 P0-10/P0-12/P0-15 处理；对象物理删除已由 OBJ-01 阻断 |
| DB-02 | `server/spacetimedb/src/reducers/hr/user.rs`、`server/spacetimedb/src/reducers/hr/role.rs`、`server/spacetimedb/src/reducers/hr/relations/cleanup.rs` | 删除一个用户或角色时事务清理其授权和关系行 | 用户/角色删除 Reducer | 单个主体及从属关系 | 是 | 高 D2 | 管理员/权限管理员、当前租户、禁止自删/系统管理员角色、关系事务 | 保留正常管理能力并登记 D2 | 复核入口权限、主体限制和事务调用 | 统一高风险二次认证与审计属于 P0-10/P0-15，不在 P0-01 扩展范围 |
| DB-03 | `server/spacetimedb/src/reducers/platform/migration/sequence.rs` 及同目录迁移 Reducer | 迁移事务暂存行、生成后立即删除占位行、再恢复原行以推进序列 | 管理员或数据库所有者迁移 Reducer | 导入批次及序列占位行 | 是，但只在显式迁移调用时 | 高 D2 | 管理员/所有者校验、租户校验、批次上限、Reducer 事务回滚 | 保留迁移内部机制；不作为生产发布自动步骤；工作流无迁移入口 | 编译/测试及静态调用链复核 | 数据迁移仍须业务数据负责人审批；系统性迁移演练和审计留给后续任务 |
| DB-04 | `server/spacetimedb/src/procedures/center/auth/sms.rs` | 验证码过期、失败次数超限或成功后删除单条挑战 | 短信登录 Procedure | 单条临时挑战 | 是 | 低 D3 | 发送者绑定、过期/次数状态、事务 | 保留必要安全清理 | 代码上下文复核 | 不属于业务数据批量删除 |
| DB-05 | `server/spacetimedb/src/reducers/**`（精确路径见附录 A） | 普通资源、纯关联行、临时配置或单根对象的 `.delete` | 各业务 Reducer | 单资源或其事务内关系 | 是 | 中 D2/D3 | 各模块的管理员、角色、租户、园区或资源守卫不一 | 不移除用户需要的单对象删除；完整登记 54 个文件、114 处直接删除原语 | `rg` 计数复核，新增生产 D0 路径由扫描阻断 | 授权与审计尚未统一，归属 P0-10/P0-12/P0-15；不是未说明的 D0 路径 |

## 3. 开发、测试、文档与误报

| 分类 | 文件路径 | 类型/行为 | 是否生产可达 | 建议与本次处理 |
|---|---|---|---|---|
| 开发专用 | `scripts/dev/spacetimedb-lan-proxy.mjs` | 网络连接错误时销毁 socket | 否，不删除文件或数据 | 误报，保留 |
| 测试代码 | `src/pages/contract/model.rs` 及 Rust `#[cfg(test)]` 用例 | 清空内存字符串/表单状态以验证校验 | 否 | 误报，保留 |
| UI 状态 | `src/pages/maintenance/overview.rs` | 园区切换时清空内存中的厂房选择 | 是，但只影响当前表单 | 误报，保留 |
| 数值处理 | `src/pages/billing/fees.rs` | 截断小数位字符串 | 是，但不删除持久化数据 | 误报，保留 |
| 文档示例 | `server/AGENTS.md`、`server/.github/copilot-instructions.md` | 可复制的数据库清空/删除示例 | 文档本身不可执行，但易被误用 | 删除可复制危险命令，改为生产永久禁止和兼容迁移说明 |
| 技术事实文档 | `server/README.md`、`server/ARCHITECTURE.md`、`server/ARCHITECTURE.html` | 说明平台在不兼容 schema 下的数据删除行为 | 不可执行 | 保留必要技术事实；移除“切换生产变量即可清库”的运维指引，标注最新决策替代 |
| 发布文档 | `README.md`、`deploy/README.md` | 描述自动生产、强制/任意发布配置 | 文档不可执行，但指导真实发布 | 改为仅手动生产、固定非破坏发布；完整发布门禁指向 P0-16 |
| Runner 临时文件 | `.github/workflows/deploy-production.yml` | build runner 结束时删除单个临时运行配置文件 | 只在临时 runner | 保留；这是精确单文件凭据清理，不是递归、业务或服务器数据删除 |
| 服务操作 | `deploy/scripts/remote_deploy.sh` | systemd restart/reload/enable | 是 | 属于正常发布可用性操作，不是数据库或文件销毁；保留 |
| 导出 SQL | `scripts/migration/export_billing_mysql.sql`、`export_factory_mysql.sql`、`export_salary_mysql.sql` | 只读导出脚本 | 不在工作流中 | 未发现 DELETE、DROP 或 TRUNCATE；保留 |
| 容器与卷 | 全仓 | 未发现 Dockerfile、Compose 文件、volume 删除或 `down -v` | 否 | 无需修改；后续新增时由扫描规则阻断生产脚本中的卷删除 |

## 4. 自动保护与负向测试

`scripts/check-production-destructive-ops.ps1` 只输出规则 ID 和文件路径，不回显匹配行或配置值。它覆盖：

1. 生产 workflow 的 push 触发；
2. 数据清空参数、数据库删除命令和旧危险变量；
3. 任意 shell 发布、递归删除和容器卷删除；
4. 手动触发、`needs: build`、`production` 上下文、固定生产标识和扫描步骤必须存在；
5. 远端脚本必须有已知环境 fail-closed 和生产直接拒绝逻辑；
6. Nginx 的公开模块 publish 与数据库 DELETE 拒绝必须同时覆盖两个公开代理入口。

自测完全使用内存字符串，不创建或删除测试文件。负向样例分别注入 push 触发、数据库数据删除参数和动态 shell，三类都必须被对应规则拒绝。

### 4.1 本地验证记录

| 检查 | 结果 | 说明 |
|---|---|---|
| `pwsh -NoProfile -File scripts/check-production-destructive-ops.ps1 -SelfTest` | 通过 | 三类内存负向样例均被拒绝 |
| `pwsh -NoProfile -File scripts/check-production-destructive-ops.ps1` | 通过 | 实际生产 workflow、远端脚本和 Nginx 配置满足规则 |
| `cargo check --workspace` | 通过 | 只有既有 unused/dead-code warnings |
| `cargo test -p parkwise` | 通过 | 189 项通过，包含 3 项对象删除策略测试 |
| `cargo test --workspace r2_cleanup -- --nocapture` | 既有环境失败，已隔离 | Windows 原生链接 `parkwise_server` 测试时缺少 SpacetimeDB 宿主 ABI 符号；主应用同名相关测试随后单独通过，失败与本次代码无关 |
| `cargo fmt --all -- --check` | 既有基线失败，已隔离 | 大量未修改业务文件已有格式差异；未为通过检查而改动无关文件 |
| `rustfmt --edition 2021 --check src/services/storage/r2_cleanup.rs` | 通过 | 本次唯一 Rust 修改文件格式正确 |
| Git for Windows `bash -n deploy/scripts/remote_deploy.sh` | 通过 | Shell 语法正确 |
| GitHub Actions YAML 本地解析 | 通过 | YAML 结构可解析；P0-01 扫描另行验证 GitHub 触发和依赖语义 |
| Nginx 静态结构检查 | 通过 | 括号平衡，两个公开代理入口都执行拒绝；本机无 Nginx 可执行文件，未运行 `nginx -t`，远端部署脚本仍在 reload 前强制执行该检查 |
| 最终 `rg` 危险入口复核 | 通过 | 剩余命中仅为拒绝规则/测试样例、技术事实文档、已登记 D2/D3、开发连接销毁和字符串截断误报 |
| `git diff --check` | 通过 | 无空白错误；Git 仅提示 Windows 工作区行尾转换信息 |
| 变更内容敏感模式扫描 | 通过 | 14 个变更路径中无环境文件、密钥/证书/数据库/构建/上传路径；新增内容未命中私钥头、常见 Token、云访问密钥、JWT 或 IP 地址模式 |

## 5. 修改后结论

- 仓库定义的生产工作流已无自动生产触发、清库变量、全库删除参数、强制发布入口或任意发布 shell。
- 远端脚本环境未知时在任何部署动作前失败；生产上下文出现破坏性配置项名称时直接失败。
- 公网代理配置直接拒绝模块发布和数据库 DELETE；固定模块发布只能经服务器本机目标执行。
- 生产/未知环境的对象物理删除直接失败，P0-05 完成 30 天恢复与审批机制前不会不可恢复删除对象。
- D2/D3 业务删除均已说明，没有被误删；其统一二次认证、影响预览和不可变审计仍由既定后续任务实施。
- 本地仓库未发现未说明的生产可达 D0 路径。由于本任务禁止连接服务器，当前运行中的服务器是否已经加载新 Nginx/脚本版本无法确认，也未进行任何部署或重启。

## 附录 A：直接数据库删除原语清单

全仓 `server/spacetimedb/src/**/*.rs` 共发现 54 个文件、114 处直接 `.delete(...)`。下列路径全部按上下文复核；除单独列出的迁移、园区、用户/角色级联和临时挑战外，其余属于 D3 单资源或从属关系删除。

- `server/spacetimedb/src/procedures/center/auth/sms.rs`
- `server/spacetimedb/src/reducers/access_control/car.rs`
- `server/spacetimedb/src/reducers/access_control/door.rs`
- `server/spacetimedb/src/reducers/access_control/visitor.rs`
- `server/spacetimedb/src/reducers/device/asset.rs`
- `server/spacetimedb/src/reducers/device/gateway.rs`
- `server/spacetimedb/src/reducers/finance/approvals/reimbursement.rs`
- `server/spacetimedb/src/reducers/finance/billing/amount.rs`
- `server/spacetimedb/src/reducers/finance/billing/carryover.rs`
- `server/spacetimedb/src/reducers/finance/billing/details.rs`
- `server/spacetimedb/src/reducers/finance/record/images.rs`
- `server/spacetimedb/src/reducers/hr/employee/device.rs`
- `server/spacetimedb/src/reducers/hr/employee/employee.rs`
- `server/spacetimedb/src/reducers/hr/employee/leave.rs`
- `server/spacetimedb/src/reducers/hr/employee/localization.rs`
- `server/spacetimedb/src/reducers/hr/employee/salary.rs`
- `server/spacetimedb/src/reducers/hr/employee/salary_media.rs`
- `server/spacetimedb/src/reducers/hr/relations/cleanup.rs`
- `server/spacetimedb/src/reducers/hr/role.rs`
- `server/spacetimedb/src/reducers/hr/user.rs`
- `server/spacetimedb/src/reducers/investment/common.rs`
- `server/spacetimedb/src/reducers/investment/lead/company.rs`
- `server/spacetimedb/src/reducers/investment/record.rs`
- `server/spacetimedb/src/reducers/investment/score/calculate.rs`
- `server/spacetimedb/src/reducers/investment/tenant.rs`
- `server/spacetimedb/src/reducers/maintenance/elevator.rs`
- `server/spacetimedb/src/reducers/maintenance/factory.rs`
- `server/spacetimedb/src/reducers/maintenance/firefighting.rs`
- `server/spacetimedb/src/reducers/maintenance/hygiene.rs`
- `server/spacetimedb/src/reducers/maintenance/repair_order.rs`
- `server/spacetimedb/src/reducers/maintenance/transformer.rs`
- `server/spacetimedb/src/reducers/platform/center/auth/password.rs`
- `server/spacetimedb/src/reducers/platform/center/system/app_version.rs`
- `server/spacetimedb/src/reducers/platform/center/system/system_key.rs`
- `server/spacetimedb/src/reducers/platform/media/bindings.rs`
- `server/spacetimedb/src/reducers/platform/media/image.rs`
- `server/spacetimedb/src/reducers/platform/migration/sequence.rs`
- `server/spacetimedb/src/reducers/platform/permissions/role_code.rs`
- `server/spacetimedb/src/reducers/platform/permissions/user_code.rs`
- `server/spacetimedb/src/reducers/platform/profile.rs`
- `server/spacetimedb/src/reducers/platform/support/feedback.rs`
- `server/spacetimedb/src/reducers/platform/system/app_version.rs`
- `server/spacetimedb/src/reducers/platform/system/system_key.rs`
- `server/spacetimedb/src/reducers/rental/assets/dormitory.rs`
- `server/spacetimedb/src/reducers/rental/assets/dormitory_media.rs`
- `server/spacetimedb/src/reducers/rental/assets/floor.rs`
- `server/spacetimedb/src/reducers/rental/assets/floor_media.rs`
- `server/spacetimedb/src/reducers/rental/contract/tenant.rs`
- `server/spacetimedb/src/reducers/rental/contract/tenant_fee.rs`
- `server/spacetimedb/src/reducers/rental/contract/tenant_floor.rs`
- `server/spacetimedb/src/reducers/rental/contract/tenant_media.rs`
- `server/spacetimedb/src/reducers/rental/contract/tenant_meter.rs`
- `server/spacetimedb/src/reducers/rental/park/deletion.rs`
- `server/spacetimedb/src/reducers/rental/park/media.rs`

## 附录 B：复核命令类别

只读盘点使用 `rg` 对以下类别交叉检索并逐项读上下文：wipe、clear、reset、purge、truncate、drop、delete/remove all、destroy、数据库 publish/delete/data-delete 参数、动态 shell、递归删除、Docker/Compose 卷删除、对象存储删除、Rust `.delete`、PowerShell 递归删除，以及 workflow、Shell、PowerShell、Nginx、systemd、SQL 和部署文档。复核输出只保留文件路径、行号和命中类型，不输出配置值。
