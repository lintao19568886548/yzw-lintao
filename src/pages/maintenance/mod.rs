//! 园区消防、配电、电梯与报修工单工作台。

mod elevator;
mod firefighting;
mod inspect;
mod model;
mod overview;
mod transformer;

pub(crate) use model::{
    expiry_days_left, latest_elevator_inspection, latest_firefighting_inspection,
    latest_inspection, today_date,
};
pub use elevator::MaintenanceElevatorPage;
pub use firefighting::MaintenanceFirefightingPage;
pub use inspect::{
    MaintenanceInspectElevatorPage, MaintenanceInspectFirefightingPage,
    MaintenanceInspectTransformerPage,
};
pub use overview::*;
pub use transformer::MaintenanceTransformerPage;
