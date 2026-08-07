//! 角色与菜单权限的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = role_menu,
    index(accessor = role_menu_by_role, btree(columns = [role_id])),
    index(accessor = role_menu_by_menu, btree(columns = [menu_id])),
    index(accessor = role_menu_by_pair, btree(columns = [role_id, menu_id]))
)]
pub struct RoleMenu {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub role_id: u64,
    pub menu_id: u64,
    /// 保留 MySQL 的软删除语义，重新授权时恢复原关系。
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
