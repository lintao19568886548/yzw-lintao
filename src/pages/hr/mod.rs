//! 人事管理页面集合。

mod attendance;
mod attendance_location;
mod employee;
mod employee_form;
mod flow;
mod leave;
mod model;
mod navigation;
mod records;
mod trajectory;

pub use attendance::HrmAttendancePunchPage;
pub use attendance_location::HrmAttendanceLocationPage;
pub use employee::HrmEmployeePage;
pub use flow::HrOverviewPage;
pub(crate) use model::{employee_binding_counts, linked_id_counts};
pub use leave::HrmLeavePage;
pub use records::HrmAttendanceRecordsPage;
pub use trajectory::HrmTrajectoryPage;
