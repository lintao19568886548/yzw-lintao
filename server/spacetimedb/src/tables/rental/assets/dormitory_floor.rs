//! 宿舍楼层表。
//!
//! 原结构把宿舍的楼层信息压在建筑那一行里，只区分「一楼」和「其他层」
//! 两桶（各有自己的层高、挂牌租金、已用房间数）。这个二分法表达不了
//! 真实情况：8 层楼里 2 楼和 8 楼被迫共用一个价格和一个占用数字，而
//! 只有 1 层的宿舍反而要填「其他层」。
//!
//! 楼层成为实体之后，每层各自带层高和挂牌租金；占用情况不再人工填写，
//! 由 `RentalTenantDormitoryFloor` 里的合同关联算出来。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = dormitory_floor,
    index(accessor = dormitory_floor_by_customer, btree(columns = [customer_id])),
    index(accessor = dormitory_floor_by_dormitory, btree(columns = [dormitory_id]))
)]
pub struct DormitoryFloor {
    #[primary_key]
    #[auto_inc]
    pub dormitory_floor_id: u64,
    pub customer_id: String,
    pub dormitory_id: u64,
    /// 第几层，从 1 开始。
    pub floor_no: i32,
    /// 这一层有多少个房间。占用几间由合同关联算出，不在这里存。
    pub room_count: i32,
    /// 每间面积（百分之一平方米）。
    pub room_area_centi_square_metres: Option<i64>,
    pub floor_height_centi_metres: Option<i64>,
    /// 对外报价，不是成交价——成交价在合同里。
    pub rent_price_cents: Option<i64>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
