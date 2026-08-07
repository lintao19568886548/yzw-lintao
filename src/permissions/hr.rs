//! 人事管理职能口径。

use crate::spacetime_bindings::role_type::Role;

/// 人事全员数据权限码，与服务端 `access::CODE_HR_MANAGE` 保持一致。
pub const CODE_HR_MANAGE: &str = "hr:manage";

/// 与 SpacetimeDB 服务端保持一致：系统管理员，或持有 `hr:manage` 权限码的账号，
/// 可以管理全员人事数据。
///
/// 这里只决定界面是否展示入口；真正的数据可见范围由服务端 view 判定，客户端
/// 即使判断错也读不到无权访问的数据。
pub fn can_manage_hr(roles: &[Role], codes: &[String]) -> bool {
    let is_system_super = roles
        .iter()
        .any(|role| role.status == 1 && role.name == "Super" && role.scope == "system");
    is_system_super || codes.iter().any(|code| code == CODE_HR_MANAGE)
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

    fn codes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn 系统管理员始终可管理全员人事数据() {
        assert!(can_manage_hr(&[role("Super", "system", 1)], &[]));
        assert!(!can_manage_hr(&[role("Super", "tenant", 1)], &[]));
    }

    #[test]
    fn 人事职能由权限码授予而非角色名() {
        // 角色叫「人事部」但没拿到权限码：不再自动获得权限。
        assert!(!can_manage_hr(&[role("人事部", "tenant", 1)], &[]));
        assert!(!can_manage_hr(&[role("董事长", "tenant", 1)], &[]));
        // 拿到权限码后，角色叫什么都不影响。
        assert!(can_manage_hr(
            &[role("随便什么名字", "tenant", 1)],
            &codes(&[CODE_HR_MANAGE])
        ));
        assert!(!can_manage_hr(
            &[role("人事部", "tenant", 1)],
            &codes(&["billing:export"])
        ));
    }
}
