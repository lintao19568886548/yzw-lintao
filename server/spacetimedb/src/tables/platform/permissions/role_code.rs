//! 角色与标准权限码的关联表。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = role_code,
    index(accessor = role_code_by_customer, btree(columns = [customer_id])),
    index(accessor = role_code_by_role, btree(columns = [role_id])),
    index(accessor = role_code_by_code, btree(columns = [code_id])),
    index(accessor = role_code_by_pair, btree(columns = [role_id, code_id]))
)]
pub struct RoleCode {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub role_id: u64,
    pub code_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
