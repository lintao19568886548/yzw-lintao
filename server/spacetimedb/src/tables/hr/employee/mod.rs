//! 员工与考勤业务表。

#[path = "attendance.rs"]
mod attendance_table;
#[path = "attendance_location.rs"]
mod attendance_location_table;
mod device;
#[path = "employee.rs"]
mod employee_table;
mod leave;
#[path = "localization.rs"]
mod localization_table;
#[path = "salary.rs"]
mod salary_table;
#[path = "salary_finance.rs"]
mod salary_finance_table;

pub use attendance_table::*;
pub use attendance_location_table::*;
pub use device::*;
pub use employee_table::*;
pub use leave::*;
pub use localization_table::*;
pub use salary_table::*;
pub use salary_finance_table::*;
