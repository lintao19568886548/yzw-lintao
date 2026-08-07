//! 设施维保业务表。

#[path = "elevator.rs"]
mod elevator_table;
#[path = "elevator_asset.rs"]
mod elevator_asset_table;
#[path = "elevator_inspection.rs"]
mod elevator_inspection_table;
#[path = "factory.rs"]
mod factory_maintenance_table;
#[path = "firefighting.rs"]
mod firefighting_table;
#[path = "firefighting_asset.rs"]
mod firefighting_asset_table;
#[path = "firefighting_inspection.rs"]
mod firefighting_inspection_table;
mod hygiene;
mod images;
#[path = "repair_order.rs"]
mod repair_order_table;
#[path = "transformer.rs"]
mod transformer_table;
#[path = "transformer_asset.rs"]
mod transformer_asset_table;
#[path = "transformer_inspection.rs"]
mod transformer_inspection_table;

pub use elevator_asset_table::*;
pub use elevator_inspection_table::*;
pub use elevator_table::*;
pub use factory_maintenance_table::*;
pub use firefighting_asset_table::*;
pub use firefighting_inspection_table::*;
pub use firefighting_table::*;
pub use hygiene::*;
pub use images::*;
pub use repair_order_table::*;
pub use transformer_asset_table::*;
pub use transformer_inspection_table::*;
pub use transformer_table::*;
