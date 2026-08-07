//! 租赁合同与厂房楼层的归属关系表。
//!
//! 建这张表之前，「这份合同租的是哪一层」只存在于 `RentalTenant.address`
//! 的自由文本里（"A栋一楼"、"佛山A栋102"），既无法程序化关联，也导致
//! 楼层的已用面积只能人工维护、跟合同面积长期对不上。
//!
//! 用关联表而不是给合同加一个 `floor_id`，是因为一份合同确实会跨多层甚至
//! 多栋——生产数据里有「A栋101，201，5楼整层，B栋101，5楼整层」这种。
//! 反过来一层也可能被多份合同分租，所以是多对多。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = rental_tenant_floor,
    index(accessor = rental_tenant_floor_by_customer, btree(columns = [customer_id])),
    index(accessor = rental_tenant_floor_by_tenant, btree(columns = [rental_tenant_id])),
    index(accessor = rental_tenant_floor_by_floor, btree(columns = [floor_id])),
    index(accessor = rental_tenant_floor_by_pair, btree(columns = [rental_tenant_id, floor_id]))
)]
pub struct RentalTenantFloor {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub rental_tenant_id: u64,
    pub floor_id: u64,
    /// 这份合同在这一层占用的面积（百分之一平方米）。
    ///
    /// 单独存而不是从楼层总面积推，因为一层可以拆给多个租户；整层出租时
    /// 它等于楼层总面积，半层出租时小于总面积。
    pub area_centi_square_metres: i64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
