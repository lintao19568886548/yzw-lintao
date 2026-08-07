//! 员工与考勤共用校验。

use spacetimedb::{DbContext, ReducerContext, Timestamp};

use crate::{
    access::{self, Duty},
    reducers::shared::access::{current_customer_id, current_user_id, require_user},
    tables::*,
};

const MICROS_PER_SECOND: i64 = 1_000_000;
const SECONDS_PER_DAY: i64 = 86_400;
const CHINA_TIME_OFFSET_SECONDS: i64 = 8 * 3_600;

pub(crate) fn current_enabled_user(ctx: &ReducerContext) -> Result<SystemUser, String> {
    let user_id = current_user_id(ctx).ok_or("当前身份未绑定用户")?;
    let user = require_user(ctx, user_id)?;
    (user.status == 1)
        .then_some(user)
        .ok_or("用户已被禁用".into())
}

/// 人事管理权限：系统管理员，或持有 `hr:manage` 权限码的账号。
///
/// 与读取侧 `Duty::HrManage` 用同一份判定，避免写入口径和可见范围各说各话。
pub(super) fn require_hr_manager(ctx: &ReducerContext) -> Result<SystemUser, String> {
    let user = current_enabled_user(ctx)?;
    let allowed = access::read_scope(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
        .is_some_and(|scope| scope.has_duty(Duty::HrManage));
    allowed.then_some(user).ok_or("需要人事管理权限".into())
}

pub(super) fn require_employee(ctx: &ReducerContext, employee_id: u64) -> Result<Employee, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .employee()
        .employee_id()
        .find(employee_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("员工不存在".into())
}

pub(super) fn require_attendance(
    ctx: &ReducerContext,
    attendance_id: u64,
) -> Result<Attendance, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .attendance()
        .attendance_id()
        .find(attendance_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("找不到该打卡记录".into())
}

/// 原系统按中国时区判断同一天，迁移期间保持固定 UTC+8 语义。
pub(super) fn local_day(timestamp: Timestamp) -> i64 {
    let seconds = timestamp
        .to_micros_since_unix_epoch()
        .div_euclid(MICROS_PER_SECOND);
    (seconds + CHINA_TIME_OFFSET_SECONDS).div_euclid(SECONDS_PER_DAY)
}

pub(super) fn local_second_of_day(timestamp: Timestamp) -> u32 {
    let seconds = timestamp
        .to_micros_since_unix_epoch()
        .div_euclid(MICROS_PER_SECOND);
    (seconds + CHINA_TIME_OFFSET_SECONDS).rem_euclid(SECONDS_PER_DAY) as u32
}

pub(super) fn validate_coordinate(longitude_e15: i64, latitude_e15: i64) -> Result<(), String> {
    const SCALE: i64 = 1_000_000_000_000_000;
    if !(-180 * SCALE..=180 * SCALE).contains(&longitude_e15)
        || !(-90 * SCALE..=90 * SCALE).contains(&latitude_e15)
    {
        return Err("定位失败，请开启定位权限后重新打卡".into());
    }
    Ok(())
}

pub(super) fn employee_schedule(ctx: &ReducerContext, user_id: u64) -> (u32, u32) {
    let customer_id = current_customer_id(ctx).unwrap_or_default();
    ctx.db
        .employee()
        .employee_by_customer()
        .filter(customer_id.as_str())
        .find(|row| row.user_id == Some(user_id) && !row.is_deleted && !row.is_resigned)
        .map(|row| {
            (
                row.check_in_seconds.unwrap_or(9 * 3_600),
                row.check_out_seconds.unwrap_or(18 * 3_600),
            )
        })
        .unwrap_or((9 * 3_600, 18 * 3_600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 中国时区日期边界按零点切换() {
        let before_midnight =
            Timestamp::from_micros_since_unix_epoch((16 * 3_600 - 1) * MICROS_PER_SECOND);
        let midnight = Timestamp::from_micros_since_unix_epoch(16 * 3_600 * MICROS_PER_SECOND);
        assert_eq!(local_day(before_midnight), 0);
        assert_eq!(local_day(midnight), 1);
        assert_eq!(local_second_of_day(Timestamp::UNIX_EPOCH), 8 * 3_600);
    }

    #[test]
    fn 经纬度定点范围校验正确() {
        assert!(validate_coordinate(180_000_000_000_000_000, 90_000_000_000_000_000).is_ok());
        assert!(validate_coordinate(180_000_000_000_000_001, 0).is_err());
        assert!(validate_coordinate(0, -90_000_000_000_000_001).is_err());
    }
}
