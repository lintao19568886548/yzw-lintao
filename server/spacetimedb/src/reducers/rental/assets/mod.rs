//! 园区资产事务逻辑。

#[path = "dormitory_media.rs"]
mod dormitory_media_reducer;
#[path = "dormitory.rs"]
mod dormitory_reducer;
#[path = "dormitory_floor.rs"]
mod dormitory_floor_reducer;
#[path = "factory.rs"]
mod factory_reducer;
#[path = "floor_media.rs"]
mod floor_media_reducer;
#[path = "floor.rs"]
mod floor_reducer;
#[path = "meter.rs"]
mod meter_reducer;

pub(crate) use dormitory_reducer::soft_delete_dormitory_tree;
pub(crate) use factory_reducer::soft_delete_factory_tree;
pub(crate) use meter_reducer::require_utility_meter;
