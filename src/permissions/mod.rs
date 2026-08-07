//! 客户端页面与导航共用的权限判断。

mod access;
mod hr;
mod management;
mod rental;
mod routes;

pub use access::can_manage_access;
pub use hr::can_manage_hr;
pub use management::can_manage_permissions;
pub use rental::can_manage_rental;
pub use routes::{can_access_route, first_accessible_path, is_system_super};
