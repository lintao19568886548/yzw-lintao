//! 园区资产层级表。

#[path = "dormitory.rs"]
mod dormitory_table;
#[path = "dormitory_floor.rs"]
mod dormitory_floor_table;
#[path = "floor.rs"]
mod factory_floor_table;
#[path = "meter.rs"]
mod utility_meter_table;
#[path = "factory.rs"]
mod factory_table;

pub use dormitory_floor_table::*;
pub use dormitory_table::*;
pub use factory_floor_table::*;
pub use factory_table::*;
pub use utility_meter_table::*;
