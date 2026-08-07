//! 标准权限码表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `code` 表。
#[spacetimedb::table(
    accessor = code,
    index(accessor = code_by_customer, btree(columns = [customer_id])),
    index(accessor = code_by_customer_value, btree(columns = [customer_id, code]))
)]
pub struct Code {
    #[primary_key]
    #[auto_inc]
    pub code_id: u64,
    pub customer_id: String,
    pub code: String,
    pub name: String,
    pub content: Option<String>,
    pub menu_id: Option<u64>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    pub template_deleted_at: Option<Timestamp>,
    pub template_internal_only: bool,
    pub template_key: Option<String>,
    pub template_managed: bool,
    pub template_version: i32,
}
