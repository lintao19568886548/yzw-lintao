//! 租赁管理角色口径。

use crate::spacetime_bindings::{menu_type::Menu, role_type::Role};

/// 原项目中进入园区管理页面的角色可以使用页面内的新增、修改、管理厂房和删除操作。
pub fn can_manage_rental(roles: &[Role], menus: &[Menu]) -> bool {
    roles
        .iter()
        .any(|role| role.status == 1 && role.name == "Super" && role.scope == "system")
        || menus.iter().any(|menu| {
            menu.status == 1
                && menu.template_deleted_at.is_none()
                && (matches!(menu.path.as_str(), "/system/park" | "/rental/manage")
                    || matches!(
                        menu.auth_code.as_deref(),
                        Some("system:park" | "rental:manage")
                    ))
        })
}

#[cfg(test)]
mod tests {
    use spacetimedb_sdk::Timestamp;

    use super::*;

    fn role(name: &str, scope: &str, status: i8) -> Role {
        Role {
            role_id: 1,
            customer_id: "public".into(),
            name: name.into(),
            remark: None,
            status,
            rates: None,
            parent_id: None,
            reimbursement_auth: None,
            organization_id: None,
            scope: scope.into(),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn menu(path: &str) -> Menu {
        Menu {
            menu_id: 1,
            customer_id: "public".into(),
            name: "园区管理".into(),
            menu_type: "menu".into(),
            status: 1,
            path: path.into(),
            active_path: None,
            redirect: None,
            component: None,
            parent_id: None,
            auth_code: Some("system:park".into()),
            template_key: None,
            template_parent_key: None,
            template_version: 1,
            template_managed: false,
            template_internal_only: false,
            template_deleted_at: None,
        }
    }

    #[test]
    fn super_或拥有园区菜单的角色可以维护园区主档() {
        assert!(can_manage_rental(&[role("Super", "system", 1)], &[]));
        assert!(can_manage_rental(
            &[role("园区管理员", "tenant", 1)],
            &[menu("/system/park")]
        ));
        assert!(!can_manage_rental(&[role("园区管理员", "tenant", 1)], &[]));
    }
}
