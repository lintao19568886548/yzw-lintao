//! 用户与角色的多对多关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = user_role,
    index(accessor = user_role_by_user, btree(columns = [user_id])),
    index(accessor = user_role_by_role, btree(columns = [role_id])),
    index(accessor = user_role_by_pair, btree(columns = [user_id, role_id]))
)]
pub struct UserRole {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub user_id: u64,
    pub role_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
