//! 人事域的写入逻辑：员工档案、账号角色、授权关系。
//!
//! 对应侧边栏「人事」分组下的人事管理、工资管理、角色管理。

#[path = "employee/mod.rs"]
pub(crate) mod employee_reducers;
pub(crate) mod relations;
#[path = "role.rs"]
pub(crate) mod role_reducer;
#[path = "user.rs"]
pub(crate) mod user_reducer;
