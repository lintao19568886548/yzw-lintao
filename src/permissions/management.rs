//! 角色权限管理入口判断。

use crate::spacetime_bindings::{menu_type::Menu, role_type::Role};

/// 与原项目一致：系统级 Super 默认放行，其他角色必须实际拥有角色管理菜单。
pub fn can_manage_permissions(roles: &[Role], menus: &[Menu]) -> bool {
    roles
        .iter()
        .any(|role| role.status == 1 && role.name == "Super" && role.scope == "system")
        || menus.iter().any(|menu| {
            menu.status == 1 && menu.template_deleted_at.is_none() && menu.path == "/system/role"
        })
}
