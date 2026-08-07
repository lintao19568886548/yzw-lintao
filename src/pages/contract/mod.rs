//! 合同管理页面模块。

mod attachments;
mod form;
mod model;
mod overview;
mod page;

pub(crate) use model::{contract_locations, meter_location_label};
pub use overview::ContractManagementPage;
pub use page::{ContractCreatePage, ContractDetailPage, ContractEditPage};
