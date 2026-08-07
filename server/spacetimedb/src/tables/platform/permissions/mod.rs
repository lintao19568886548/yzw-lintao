//! 权限码及其授权关系表。

#[path = "code.rs"]
mod permission_code;
#[path = "role_code.rs"]
mod role_code_table;
#[path = "user_code.rs"]
mod user_code_table;

pub use permission_code::*;
pub use role_code_table::*;
pub use user_code_table::*;
