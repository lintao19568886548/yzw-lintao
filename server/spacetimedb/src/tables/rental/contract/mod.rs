//! 租赁业务表。

#[path = "tenant.rs"]
mod rental_tenant_table;
#[path = "tenant_dormitory_floor.rs"]
mod rental_tenant_dormitory_floor_table;
#[path = "tenant_fee.rs"]
mod rental_tenant_fee_table;
#[path = "tenant_floor.rs"]
mod rental_tenant_floor_table;
#[path = "tenant_meter.rs"]
mod rental_tenant_meter_table;

pub use rental_tenant_dormitory_floor_table::*;
pub use rental_tenant_fee_table::*;
pub use rental_tenant_floor_table::*;
pub use rental_tenant_meter_table::*;
pub use rental_tenant_table::*;
