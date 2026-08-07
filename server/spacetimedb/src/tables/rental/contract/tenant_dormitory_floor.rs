//! 租赁合同与宿舍楼层的归属关系表。
//!
//! 记的是「这份合同在这一层占了几间」，而不是精确到房号。这样整层出租
//! （占满该层全部房间）和按间出租（占其中几间）用同一套结构就能表达；
//! 真实数据里还有「211房（靠左）」「211房（靠右）」这种同一房号拆成
//! 两份合同的情况，按间数统计不会因此算错。
//!
//! 需要精确到房号时，在楼层下面再加一层房间表即可，不必推翻这里。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = rental_tenant_dormitory_floor,
    index(
        accessor = rental_tenant_dormitory_floor_by_customer,
        btree(columns = [customer_id])
    ),
    index(
        accessor = rental_tenant_dormitory_floor_by_tenant,
        btree(columns = [rental_tenant_id])
    ),
    index(
        accessor = rental_tenant_dormitory_floor_by_floor,
        btree(columns = [dormitory_floor_id])
    )
)]
pub struct RentalTenantDormitoryFloor {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub rental_tenant_id: u64,
    pub dormitory_floor_id: u64,
    /// 这份合同在这一层占用的房间数。
    pub room_count: i32,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
