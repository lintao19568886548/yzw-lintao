//! 厂房维护记录表定义。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = factory_maintenance,
    index(accessor = factory_maintenance_by_customer, btree(columns = [customer_id])),
    index(accessor = factory_maintenance_by_factory, btree(columns = [factory_id])),
    index(accessor = factory_maintenance_by_park, btree(columns = [park_id]))
)]
pub struct FactoryMaintenance {
    #[primary_key]
    #[auto_inc]
    pub factory_maintenance_id: u64,
    pub customer_id: String,
    pub maintenance_item: String,
    pub maintenance_status: String,
    pub person_in_charge: String,
    pub start_time: Timestamp,
    pub end_time: Option<Timestamp>,
    pub remark: Option<String>,
    pub factory_id: u64,
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
