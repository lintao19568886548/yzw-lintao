//! 宿舍楼表定义。
//!
//! 表名是 `dormitory_building` 而不是 `dormitory`：这次要删掉一批列，而
//! SpacetimeDB 不允许在已有表上增删列（哪怕表是空的）。换个表名后变成
//! 「删旧表 + 建新表」，落在自动迁移允许的范围内。
//!
//! 相对原结构删掉了三类字段，原因分别是：
//! - `used_rooms_first` / `used_rooms_other`：占用情况是可以从合同算出来的
//!   （楼层房间总数固定，减去合同占用的即为可租），人工填必然漂移——
//!   厂房那边同性质的字段已经让两套面积差出 18 万平米。
//! - `rent_price_first` / `rent_price_other`、
//!   `floor_height_first` / `floor_height_other`：「一楼 vs 其他层」这个
//!   二分法表达不了 8 层楼里各层的差异，改由 `DormitoryFloor` 逐层承载。
//! - `total_rooms`：等于各层房间数之和，同样是派生值。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = dormitory_building,
    index(accessor = dormitory_building_by_customer, btree(columns = [customer_id])),
    index(accessor = dormitory_building_by_park, btree(columns = [park_id]))
)]
pub struct Dormitory {
    #[primary_key]
    #[auto_inc]
    pub dormitory_id: u64,
    pub customer_id: String,
    pub park_id: u64,
    pub dormitory_name: String,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
