//! 园区管理页面模块。

mod detail;
mod flow;
mod form;
mod model;
mod overview;

pub use flow::RentalOverviewPage;
pub(crate) use form::ParkProfileDialog;
pub(crate) use model::{
    factory_park_link_counts, floor_factory_link_counts, floor_used_areas,
    is_active_income_contract, park_areas, unmatched_contract_count, ParkAreas,
};
pub use overview::RentalManagementPage;
