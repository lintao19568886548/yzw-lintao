//! 定位打卡记录表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `localization` 表。
#[spacetimedb::table(
    accessor = localization,
    index(accessor = localization_by_customer, btree(columns = [customer_id])),
    index(accessor = localization_by_customer_user, btree(columns = [customer_id, user_id])),
    index(accessor = localization_by_user_time, btree(columns = [user_id, punch_time]))
)]
pub struct Localization {
    #[primary_key]
    #[auto_inc]
    pub localization_id: u64,
    pub customer_id: String,
    pub punch_time: Timestamp,
    pub user_name: String,
    pub status: i32,
    /// MySQL `decimal(18,15)` 乘以 `10^15` 后保存，避免浮点误差。
    pub longitude_e15: i64,
    /// MySQL `decimal(18,15)` 乘以 `10^15` 后保存，避免浮点误差。
    pub latitude_e15: i64,
    pub user_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
