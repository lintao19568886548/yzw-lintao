//! 用户反馈表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `feedback` 表。
#[spacetimedb::table(
    accessor = feedback,
    index(accessor = feedback_by_customer, btree(columns = [customer_id])),
    index(accessor = feedback_by_customer_user, btree(columns = [customer_id, user_id])),
    index(accessor = feedback_by_center_user, btree(columns = [center_user_id]))
)]
pub struct Feedback {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub category: String,
    pub content: String,
    pub contact: Option<String>,
    pub client_platform: Option<String>,
    pub source: String,
    pub user_agent: Option<String>,
    pub user_id: u64,
    pub username: String,
    pub real_name: String,
    pub center_user_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
