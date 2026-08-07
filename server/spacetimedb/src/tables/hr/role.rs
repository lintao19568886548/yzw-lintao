//! 角色表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `role` 表。
#[spacetimedb::table(
    accessor = role,
    index(accessor = role_by_customer, btree(columns = [customer_id])),
    index(accessor = role_by_scope, btree(columns = [scope]))
)]
pub struct Role {
    #[primary_key]
    #[auto_inc]
    pub role_id: u64,
    pub customer_id: String,
    pub name: String,
    pub remark: Option<String>,
    pub status: i8,
    pub rates: Option<i32>,
    /// 父角色允许为空；删除角色前由 reducer 检查子角色。
    pub parent_id: Option<u64>,
    pub reimbursement_auth: Option<i32>,
    pub organization_id: Option<u64>,
    pub scope: String,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
