//! 园区档案与待租厂房。
//!
//! 园区的浏览入口只有园区管理一处（`/rental/manage`），这里不再有单独的
//! 园区列表页——两者展示的是同一批园区，只是维度不同，重复维护会漂移。

mod asset_form;
mod detail;
mod detail_asset_edit;
mod detail_model;
mod detail_sections;
mod meter_section;
mod model;
mod vacant;
mod vacant_detail;

pub(crate) use meter_section::{
    decode_location, encode_location, meter_location_options, PUBLIC_AREA,
};

pub use detail::RentalParkDetailPage;
pub use vacant::VacantFactoryPage;
pub use vacant_detail::VacantFactoryDetailPage;
