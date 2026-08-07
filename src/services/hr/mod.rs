//! 人事管理客户端服务。

mod attendance;
mod attendance_location;
mod employee;
mod leave;

pub use attendance::{punch_in_record, punch_out_record};
pub use attendance_location::{
    create_attendance_location_record, delete_attendance_location_record,
    update_attendance_location_record,
};
pub use employee::{create_employee_record, delete_employee_record, update_employee_record};
pub use leave::{
    audit_leave_record, create_leave_record, delete_leave_record, update_leave_record,
};

pub(super) fn finish_reducer<E: std::fmt::Display>(
    result: Result<Result<(), String>, E>,
    action: &'static str,
) -> Result<(), String> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(message),
        Err(error) => Err(format!("{action}失败：{error}")),
    }
}
