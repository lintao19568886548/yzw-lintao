//! 中心应用版本发布表。

use spacetimedb::Timestamp;

/// 对应中心库 `app_versions` 表。
#[spacetimedb::table(
    accessor = center_app_version,
    index(accessor = center_app_version_by_scope, btree(columns = [center_scope])),
    index(accessor = center_app_version_by_version, btree(columns = [version])),
    index(accessor = center_app_version_by_time, btree(columns = [created_at]))
)]
pub struct CenterAppVersion {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// 中心库全局分区标识，固定为 `0`。
    pub center_scope: u8,
    pub version: String,
    pub url: Option<String>,
    pub android_url: String,
    pub ios_url: Option<String>,
    pub notes: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
