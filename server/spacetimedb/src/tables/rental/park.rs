//! 园区表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `park` 表。
///
/// 没有面积字段：园区面积一律按楼层台账算出（厂房逐层求和、宿舍「房间数 ×
/// 单间面积」，两个口径分开显示），原来那个人工填写的
/// `area_centi_square_metres` 已经删掉——它是手填的，而界面从来都是重算后
/// 覆盖它显示，两个数一旦不一致没人知道该信哪个。
#[spacetimedb::table(
    accessor = park,
    index(accessor = park_by_customer, btree(columns = [customer_id]))
)]
pub struct Park {
    #[primary_key]
    #[auto_inc]
    pub park_id: u64,
    pub customer_id: String,
    pub park_name: String,
    pub address: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub contact: Option<String>,
    pub manager: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
