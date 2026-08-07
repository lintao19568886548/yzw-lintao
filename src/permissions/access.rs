//! 门禁记录维护权限。

use crate::spacetime_bindings::role_type::Role;

/// 与服务端 Reducer 保持一致，仅系统级 Super 可以新增、修改和删除门禁记录。
pub fn can_manage_access(roles: &[Role]) -> bool {
    roles
        .iter()
        .any(|role| role.status == 1 && role.name == "Super" && role.scope == "system")
}
