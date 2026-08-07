//! 公告采集数据库中的公告表。

use spacetimedb::Timestamp;

/// 对应线上 `spider.notices` 表。
#[spacetimedb::table(
    accessor = notice,
    index(accessor = notice_by_scope, btree(columns = [global_scope])),
    index(accessor = notice_by_validity, btree(columns = [is_valid])),
    index(accessor = notice_by_last_check, btree(columns = [last_checked_at])),
    index(accessor = notice_by_date, btree(columns = [date]))
)]
pub struct Notice {
    #[primary_key]
    pub notice_id: String,
    /// 公告库全局分区标识，固定为 `0`。
    pub global_scope: u8,
    pub title: Option<String>,
    pub date: Option<String>,
    pub notice_type: Option<String>,
    pub category: Option<String>,
    pub project_type: Option<String>,
    pub site_code: Option<String>,
    pub owner: Option<String>,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub is_valid: bool,
    pub invalid_reason: Option<String>,
    pub last_checked_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
