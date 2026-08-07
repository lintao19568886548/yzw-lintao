//! 客户端应用版本表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `app_versions` 表。
#[spacetimedb::table(
    accessor = app_version,
    index(accessor = app_version_by_customer, btree(columns = [customer_id])),
    index(accessor = app_version_by_customer_version, btree(columns = [customer_id, version])),
    index(accessor = app_version_by_customer_time, btree(columns = [customer_id, created_at]))
)]
pub struct AppVersion {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub version: String,
    pub url: Option<String>,
    pub android_url: String,
    pub ios_url: Option<String>,
    pub notes: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
