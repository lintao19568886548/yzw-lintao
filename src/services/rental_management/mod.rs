//! 租赁管理客户端服务。

mod dormitory;
mod dormitory_floor;
mod factory;
mod floor;
mod meter;
mod park;

pub use dormitory::{
    create_dormitory_record, delete_dormitory_record, update_dormitory_with_images_record,
};
pub use dormitory_floor::{
    create_dormitory_floor_record, delete_dormitory_floor_record, update_dormitory_floor_record,
};
pub use factory::{
    create_factory_with_floors_record, delete_factory_record, update_factory_record,
};
pub use floor::{
    create_factory_floor_record, delete_factory_floor_record,
    update_factory_floor_with_images_record,
};
pub use meter::{
    create_utility_meter_record, delete_utility_meter_record, update_utility_meter_record,
};
pub use park::{
    create_park_with_images_record, delete_park_record, update_park_with_images_record,
};
