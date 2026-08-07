//! 人事域的表：员工档案、账号角色、授权关系。
//!
//! 对应侧边栏「人事」分组下的人事管理、工资管理、角色管理。角色和账号放在这里
//! 而不是平台层，依据是产品把「角色管理」挂在人事菜单下——租户自己管自己的人
//! 和这些人能干什么，是同一件事。

// 目录叫 employee，模块名不能也叫 employee——`#[table(accessor = employee)]`
// 会在同一个命名空间里生成一个同名项，模块会把它遮蔽掉，glob 再导出就漏了。
// 同样的规避在 finance_tables、park_table 等处已有先例。
#[path = "employee/mod.rs"]
mod employee_tables;
mod relations;
#[path = "role.rs"]
mod role_table;
#[path = "user.rs"]
mod user_table;

pub use employee_tables::*;
pub use relations::*;
pub use role_table::*;
pub use user_table::*;
