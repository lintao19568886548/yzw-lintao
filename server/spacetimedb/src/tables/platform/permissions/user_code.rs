//! 用户直接权限码表。

use spacetimedb::Timestamp;

/// MySQL 的 `user_code` 保存权限字符串，并不关联 `code.code_id`。
#[spacetimedb::table(
    accessor = user_code,
    index(accessor = user_code_by_customer, btree(columns = [customer_id])),
    index(accessor = user_code_by_user, btree(columns = [user_id])),
    index(accessor = user_code_by_pair, btree(columns = [user_id, code]))
)]
pub struct UserCode {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub user_id: u64,
    pub code: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
