//! 用户、角色、菜单和园区之间的关联表。

#[path = "role_menu.rs"]
mod role_menu_table;
#[path = "role_park.rs"]
mod role_park_table;
#[path = "user_park.rs"]
mod user_park_table;
#[path = "user_role.rs"]
mod user_role_table;

pub use role_menu_table::*;
pub use role_park_table::*;
pub use user_park_table::*;
pub use user_role_table::*;
