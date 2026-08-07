//! 菜单与路由元数据表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `menu` 表。
#[spacetimedb::table(
    accessor = menu,
    index(accessor = menu_by_customer, btree(columns = [customer_id])),
    index(accessor = menu_by_parent, btree(columns = [parent_id]))
)]
pub struct Menu {
    #[primary_key]
    #[auto_inc]
    pub menu_id: u64,
    pub customer_id: String,
    pub name: String,
    pub menu_type: String,
    pub status: i32,
    pub path: String,
    pub active_path: Option<String>,
    pub redirect: Option<String>,
    pub component: Option<String>,
    pub parent_id: Option<u64>,
    pub auth_code: Option<String>,
    pub template_key: Option<String>,
    pub template_parent_key: Option<String>,
    pub template_version: i32,
    pub template_managed: bool,
    pub template_internal_only: bool,
    pub template_deleted_at: Option<Timestamp>,
}
