//! 角色与园区的继承授权关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = role_park,
    index(accessor = role_park_by_role, btree(columns = [role_id])),
    index(accessor = role_park_by_park, btree(columns = [park_id])),
    index(accessor = role_park_by_pair, btree(columns = [role_id, park_id]))
)]
pub struct RolePark {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub role_id: u64,
    pub park_id: u64,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
