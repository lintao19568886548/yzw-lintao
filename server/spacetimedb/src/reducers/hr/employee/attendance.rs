//! 上下班打卡事务逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::{
    attendance_location::require_punch_in_range,
    common::{
        current_enabled_user, employee_schedule, local_day, local_second_of_day,
        require_attendance, validate_coordinate,
    },
    device::{AttendanceDeviceInput, prepare_device, record_device_abnormal},
};
use crate::{reducers::shared::access::require_admin, tables::*};

/// 上下班打卡位置与设备参数。
#[derive(SpacetimeType)]
pub struct AttendancePunchInput {
    pub punch_time: Timestamp,
    pub longitude_e15: i64,
    pub latitude_e15: i64,
    pub device: AttendanceDeviceInput,
}

#[spacetimedb::reducer]
pub fn punch_in(ctx: &ReducerContext, input: AttendancePunchInput) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    validate_coordinate(input.longitude_e15, input.latitude_e15)?;
    require_punch_in_range(
        ctx,
        &user.customer_id,
        input.longitude_e15,
        input.latitude_e15,
    )?;
    let duplicate = ctx
        .db
        .attendance()
        .attendance_by_customer()
        .filter(user.customer_id.as_str())
        .any(|row| {
            row.user_id == Some(user.id) && local_day(row.punch_in) == local_day(input.punch_time)
        });
    if duplicate {
        return Err("今天已经打过上班卡了".into());
    }
    let decision = prepare_device(ctx, &user, input.device)?;
    let (scheduled_check_in, _) = employee_schedule(ctx, user.id);
    let status = i8::from(local_second_of_day(input.punch_time) > scheduled_check_in);
    let attendance = ctx.db.attendance().insert(Attendance {
        attendance_id: 0,
        customer_id: user.customer_id.clone(),
        punch_in: input.punch_time,
        punch_out: None,
        status: Some(status),
        longitude_e15: input.longitude_e15,
        latitude_e15: input.latitude_e15,
        user_id: Some(user.id),
        username: user.real_name.clone(),
        punch_in_source: Some(PUNCH_SOURCE_GPS.into()),
        punch_out_source: None,
    });
    record_device_abnormal(
        ctx,
        user.id,
        attendance.attendance_id,
        "punch_in",
        input.punch_time,
        decision,
    );
    Ok(())
}

#[spacetimedb::reducer]
pub fn punch_out(
    ctx: &ReducerContext,
    attendance_id: u64,
    input: AttendancePunchInput,
) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    validate_coordinate(input.longitude_e15, input.latitude_e15)?;
    // 下班同样校验：只管上班卡的话，人可以到点了在任何地方点下班。
    require_punch_in_range(
        ctx,
        &user.customer_id,
        input.longitude_e15,
        input.latitude_e15,
    )?;
    let mut attendance = require_attendance(ctx, attendance_id)?;
    if attendance.user_id != Some(user.id) && require_admin(ctx).is_err() {
        return Err("没有权限修改他人考勤记录".into());
    }
    if attendance.punch_out.is_some() {
        return Err("该考勤记录已经完成下班打卡".into());
    }
    if input.punch_time <= attendance.punch_in {
        return Err("下班打卡时间必须晚于上班打卡时间".into());
    }
    if local_day(input.punch_time) != local_day(attendance.punch_in) {
        return Err("上下班打卡必须属于同一天".into());
    }
    let decision = prepare_device(ctx, &user, input.device)?;
    let owner_id = attendance.user_id.unwrap_or(user.id);
    let (_, scheduled_check_out) = employee_schedule(ctx, owner_id);
    let was_late = attendance.status == Some(1);
    let left_early = local_second_of_day(input.punch_time) < scheduled_check_out;
    attendance.status = Some(match (was_late, left_early) {
        (true, true) => 3,
        (false, true) => 2,
        (true, false) => 1,
        (false, false) => 0,
    });
    attendance.punch_out = Some(input.punch_time);
    attendance.longitude_e15 = input.longitude_e15;
    attendance.latitude_e15 = input.latitude_e15;
    attendance.punch_out_source = Some(PUNCH_SOURCE_GPS.into());
    ctx.db.attendance().attendance_id().update(attendance);
    record_device_abnormal(
        ctx,
        user.id,
        attendance_id,
        "punch_out",
        input.punch_time,
        decision,
    );
    Ok(())
}
