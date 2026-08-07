//! 招商记录与共享图片的关系表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `investment_image` 表。
#[spacetimedb::table(
    accessor = investment_image,
    index(accessor = investment_image_by_investment, btree(columns = [investment_id])),
    index(accessor = investment_image_by_image, btree(columns = [img_id])),
    index(accessor = investment_image_by_pair, btree(columns = [investment_id, img_id]))
)]
pub struct InvestmentImage {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub investment_id: u64,
    pub img_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
