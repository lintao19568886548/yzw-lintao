//! 共享图片主表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `image` 表。
#[spacetimedb::table(
    accessor = image,
    index(accessor = image_by_customer, btree(columns = [customer_id])),
    index(accessor = image_by_customer_hash, btree(columns = [customer_id, hash]))
)]
pub struct Image {
    #[primary_key]
    #[auto_inc]
    pub img_id: u64,
    pub customer_id: String,
    pub img_url: String,
    pub hash: String,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
