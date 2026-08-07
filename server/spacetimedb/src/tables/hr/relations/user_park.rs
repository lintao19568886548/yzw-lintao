//! 用户与园区的直接授权关系。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = user_park,
    index(accessor = user_park_by_user, btree(columns = [user_id])),
    index(accessor = user_park_by_park, btree(columns = [park_id])),
    index(accessor = user_park_by_pair, btree(columns = [user_id, park_id]))
)]
pub struct UserPark {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub user_id: u64,
    pub park_id: u64,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
