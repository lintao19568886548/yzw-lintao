# 易租云角色与权限体系

本文说明原 `vben-admin` 的角色体系、当前 SpacetimeDB 实现、两者差异，以及新增页面时必须遵守的授权规则。

## 1. 角色体系的目标

角色体系解决的不是单一的“管理员或普通用户”判断，而是以下五类问题：

1. 当前账号属于哪个租户或组织。
2. 当前账号拥有哪些角色。
3. 角色可以进入哪些菜单和页面。
4. 角色可以执行哪些按钮级操作。
5. 角色可以查看哪些园区和业务数据。

因此，角色名称本身不能代替完整权限判断。除 `Super` 特例外，业务代码应优先检查菜单关系、权限码和数据范围。

## 2. 原项目的 RBAC 关系

原项目使用数据库驱动的 RBAC，不是固定角色枚举：

```text
中心账号
   │
   ├── 租户映射 ──> 租户业务用户
   │                    │
   │                    ├── user_role ──> role
   │                    │                  ├── role_menu ──> menu
   │                    │                  ├── role_code ──> code
   │                    │                  └── role_park ──> park
   │                    │
   │                    ├── user_code ──> 用户直接权限码
   │                    └── user_park ──> 用户直接园区范围
   │
   └── 组织成员关系 ──> organization 范围角色
```

一个用户可以拥有多个角色；一个角色也可以授权给多个用户。

## 3. 角色范围

### 3.1 `system`

`scope = "system"` 表示租户业务库范围的角色。

典型用途：

- 系统管理员；
- 招商、财务、人事、园区运营等业务角色；
- 按菜单、权限码和园区范围控制业务能力。

### 3.2 `organization`

`scope = "organization"` 表示某个组织空间内部的角色，必须同时带有 `organization_id`。

原项目会为组织创建默认的“员工”角色。组织所有者、组织成员和邀请加入后的角色都应限制在对应组织内，不能跨组织复用。

### 3.3 SpacetimeDB 的租户分区

原 MySQL 使用多个租户业务库隔离数据，因此业务库 `role` 表不需要 `customer_id`。

当前 SpacetimeDB 将多租户数据放入同一个 Module，所以 `Role`、`Menu`、`SystemUser` 等表增加了 `customer_id`。所有角色、菜单和授权 Reducer 都必须校验调用者当前 `customer_id`，禁止跨租户关联主键。

## 4. 角色层级

`Role.parent_id` 表示父角色，允许形成角色树。

原项目的约束是：

- 顶级角色的 `parent_id` 为空；
- 子角色的菜单权限不能超过父角色；
- 角色列表按父子关系递归显示；
- 删除父角色前必须处理子角色；
- 不能形成循环父子关系。

因此，`parent_id` 不是仅用于界面展示，它还是子角色授权上限。

## 5. 四种授权关系

### 5.1 用户角色：`UserRole`

决定用户拥有哪些角色。一个用户可以拥有多个角色，最终权限取多个角色的并集。

相关代码：

- `../../tables/relations/user_role.rs`
- `../relations/user_role.rs`

### 5.2 角色菜单：`RoleMenu`

决定非超级管理员可以看到哪些侧边栏菜单和进入哪些页面。

`is_deleted` 使用软删除语义。重新授权时应恢复原关系，而不是无限插入重复记录。

相关代码：

- `../../tables/relations/role_menu.rs`
- `../relations/role_menu.rs`
- `../../views/permissions.rs`

### 5.3 角色权限码：`RoleCode`

控制按钮和操作级权限，例如新增、删除、审核、退款等。菜单可见不代表拥有菜单内的全部操作权限。

相关代码：

- `../../tables/permissions/role_code.rs`
- `role_code.rs`

### 5.4 角色园区：`RolePark`

限制角色可以查看和操作的园区数据。普通角色不应因为能够进入页面就获得全部园区数据。

相关代码：

- `../../tables/relations/role_park.rs`
- `../relations/park_access.rs`

## 6. 用户直接授权

原项目还支持不经过角色的用户直接授权：

- `UserCode`：直接授予用户权限码；
- `UserPark`：直接授予用户园区范围。

计算最终权限时，应合并角色继承权限和用户直接权限。`my_codes` 会使用 `source = "role"` 或 `source = "user"` 标明权限来源。

## 7. `Super` 特殊规则

原项目把角色名严格等于 `Super` 的账号视为超级管理员。

原始行为：

- `Super` 获取全部菜单，不依赖 `role_menu`；
- 可以查看跨销售、跨普通角色的数据；
- 可以管理角色、菜单和组织权限。

当前 SpacetimeDB 的 `require_admin` 还要求：

```text
role.name == "Super"
role.scope == "system"
role.status == 1
```

不能用“角色名包含管理员”代替服务端管理员校验。

## 8. 业务角色名称

除 `Super` 外，原项目允许管理员自行创建角色。下面名称是业务代码识别的角色别名，不是固定枚举：

| 业务范围 | 常见角色名称或别名 |
| --- | --- |
| 超级管理 | `Super`、超管、超级管理员、老板、董事长、总经理 |
| 招商 | 招商、招商经理、招商部、`Investment` |
| 财务 | 财务、`Finance` |
| 人事 | 人事、人事部、`HR` |
| 园区运营 | 园区经理、园区、运营、项目经理、物业 |
| 维修维护 | 维护、维修、维保、`Maintenance` |
| 门禁安保 | 门禁、前台、总台、安保 |
| 组织默认成员 | 员工，且 `scope = "organization"` |

新业务不得仅依靠这些模糊别名执行高风险写操作。别名适合待办分类和界面提示；真正授权应使用角色关系和权限码。

## 9. 菜单计算流程

当前客户端侧边栏订阅 `my_menus`：

```text
当前 SpacetimeDB Identity
  -> 当前中心用户
  -> 当前租户业务用户
  -> UserRole
  -> RoleMenu
  -> Menu
  -> Dioxus Sidebar
```

动态业务页面只创建 Dioxus 路由不会自动出现在侧边栏。页面要显示动态入口，必须同时满足：

1. `Menu.status == 1`；
2. `template_deleted_at` 为空；
3. `template_internal_only == false`；
4. 当前用户通过角色获得有效 `RoleMenu`；
5. 菜单路径与 Dioxus `Route` 一致。

公开 H5 页面是一类例外：面向匿名访客的页面不应进入后台侧边栏，也不应依赖后台角色。

## 10. 当前迁移状态与差异

截至 2026-07-12，SpacetimeDB 已迁移：

- `Role`、`UserRole`、`RoleMenu`、`RoleCode`、`RolePark`；
- `UserCode`、`UserPark`；
- `my_roles`、`my_menus`、`my_codes`；
- 管理员校验和租户主键校验。

仍需注意以下差异：

1. 当前远程数据库的角色数据只是迁移阶段快照，不代表原 MySQL 的完整角色清单。
2. `Super/system` 自动获得当前租户全部启用菜单；普通角色严格通过 `RoleMenu` 获取菜单。
3. 其他动态业务页面仍需继续迁移原 MySQL 的完整 `RoleMenu` 数据。
4. 业务角色别名尚未统一为权限码，部分业务范围判断仍需要继续迁移。

## 11. 新增后台页面的检查清单

新增或迁移页面时必须依次完成：

1. 在 Dioxus `Route` 中声明真实路径。
2. 在 `Menu` 中创建相同路径的菜单。
3. 明确哪些角色获得 `RoleMenu`，不得默认授权所有角色。
4. 为新增、修改、删除、审核等操作定义权限码。
5. 在 Reducer 中校验身份、租户、角色或权限码。
6. 在 View 中限制租户、园区和本人数据范围。
7. 使用普通角色和 `Super` 分别验证菜单与数据范围。
8. 对公开页面确认它不订阅后台敏感表。

## 12. 远程核对命令

以下命令只读，不会修改数据：

```sh
spacetime sql --server self-hosted yizu-server-yz18m \
  "SELECT role_id, customer_id, name, scope, status FROM my_roles"

spacetime sql --server self-hosted yizu-server-yz18m \
  "SELECT menu_id, name, path, status FROM my_menus"
```

执行写入命令前必须先确认目标租户和预期授权角色。
