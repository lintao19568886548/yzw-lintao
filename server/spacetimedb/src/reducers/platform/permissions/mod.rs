//! 权限码管理与授权事务。
//!
//! 完整角色体系、授权关系和迁移边界见同目录 `README.md`。

pub(crate) mod hr_manage;
pub(crate) mod maintenance_inspect;
#[path = "code.rs"]
mod permission_code;
mod role_code;
mod user_code;
