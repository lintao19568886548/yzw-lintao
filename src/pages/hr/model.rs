//! 人事页面共享的日期、时间、状态与权限计算。

use spacetimedb_sdk::Timestamp;

use crate::components::badge::BadgeVariant;

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 28_800_000_000;

#[cfg(target_arch = "wasm32")]
pub(super) fn now_timestamp() -> Timestamp {
    // 浏览器 WASM 不实现 SystemTime，使用 JavaScript 时间避免打开人事表单时触发 panic。
    let micros = (js_sys::Date::now() * 1_000.0).min(i64::MAX as f64) as i64;
    Timestamp::from_micros_since_unix_epoch(micros)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn now_timestamp() -> Timestamp {
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .min(i64::MAX as u128) as i64;
    Timestamp::from_micros_since_unix_epoch(micros)
}

pub(super) fn today() -> String {
    crate::pages::smart_meter::date::today()
}

#[pure_function::pure]
pub(super) fn parse_date(value: &str) -> Result<Timestamp, String> {
    let mut parts = value.trim().split('-');
    let y = parts.next().and_then(|v| v.parse::<i32>().ok());
    let m = parts.next().and_then(|v| v.parse::<u32>().ok());
    let d = parts.next().and_then(|v| v.parse::<u32>().ok());
    if parts.next().is_some() {
        return Err("日期格式不正确".into());
    }
    let days = y
        .zip(m)
        .zip(d)
        .and_then(|((y, m), d)| days_from_civil(y, m, d))
        .ok_or("请选择正确日期")?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    ))
}

#[pure_function::pure]
pub(super) fn parse_datetime(value: &str) -> Result<Timestamp, String> {
    let (date, time) = value.split_once('T').ok_or("请选择完整时间")?;
    let base = parse_date(date)?;
    let mut parts = time.split(':');
    let hour = parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 24)
        .ok_or("时间格式不正确")?;
    let minute = parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 60)
        .ok_or("时间格式不正确")?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        base.to_micros_since_unix_epoch() + (hour * 3600 + minute * 60) * 1_000_000,
    ))
}

#[pure_function::pure]
pub(super) fn format_date(value: Option<Timestamp>) -> String {
    value
        .map(|value| {
            let (y, m, d) = civil_from_days(
                (value.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS)
                    .div_euclid(MICROS_PER_DAY),
            );
            format!("{y:04}-{m:02}-{d:02}")
        })
        .unwrap_or_else(|| "--".into())
}

#[pure_function::pure]
pub(super) fn format_datetime(value: Option<Timestamp>) -> String {
    let Some(value) = value else {
        return "--".into();
    };
    let local = value.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let (y, m, d) = civil_from_days(local.div_euclid(MICROS_PER_DAY));
    let seconds = local.rem_euclid(MICROS_PER_DAY) / 1_000_000;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        seconds / 3600,
        seconds % 3600 / 60
    )
}

#[pure_function::pure]
pub(super) fn seconds_to_time(value: Option<u32>) -> String {
    value
        .map(|value| format!("{:02}:{:02}", value / 3600, value % 3600 / 60))
        .unwrap_or_default()
}

#[pure_function::pure]
pub(super) fn parse_time(value: &str) -> Result<Option<u32>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let mut parts = value.split(':');
    let hour = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v < 24)
        .ok_or("时间格式不正确")?;
    let minute = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v < 60)
        .ok_or("时间格式不正确")?;
    Ok(Some(hour * 3600 + minute * 60))
}

/// 考勤状态到文案与徽章。正常不着色，各类异常用危险色，请假与待完成描边。
#[pure_function::pure]
pub(super) fn attendance_status(value: Option<i8>) -> (&'static str, BadgeVariant) {
    match value {
        Some(0) => ("正常", BadgeVariant::Secondary),
        Some(1) => ("迟到", BadgeVariant::Destructive),
        Some(2) => ("早退", BadgeVariant::Destructive),
        Some(3) => ("迟到且早退", BadgeVariant::Destructive),
        Some(4) => ("缺勤", BadgeVariant::Destructive),
        Some(5) => ("请假", BadgeVariant::Outline),
        _ => ("待完成", BadgeVariant::Outline),
    }
}

/// 打卡来源到文案。
///
/// `None` 表示这条记录写在「打卡来源」这一列存在之前，而那时只有手机打卡一个
/// 入口——所以按手机定位算。这不是猜测，是当时系统里唯一的可能。
/// 未打下班卡的行传进来也是 `None`，调用方负责先判断有没有下班时间。
#[pure_function::pure]
pub(super) fn punch_source_label(value: Option<&str>) -> &'static str {
    match value {
        Some("face") => "人脸识别",
        _ => "手机定位",
    }
}
#[pure_function::pure]
pub(super) fn leave_status(value: i8) -> (&'static str, BadgeVariant) {
    match value {
        1 => ("已批准", BadgeVariant::Secondary),
        2 => ("已驳回", BadgeVariant::Destructive),
        _ => ("待审批", BadgeVariant::Outline),
    }
}

fn leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
fn mdays(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}
fn days_from_civil(y: i32, m: u32, d: u32) -> Option<i64> {
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > mdays(y, m) {
        return None;
    }
    let mut y = i64::from(y);
    let m = i64::from(m);
    let d = i64::from(d);
    y -= i64::from(m <= 2);
    let e = y.div_euclid(400);
    let yo = y - e * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    Some(e * 146097 + (yo * 365 + yo / 4 - yo / 100 + (153 * mp + 2) / 5 + d - 1) - 719468)
}
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let e = z.div_euclid(146097);
    let doe = z - e * 146097;
    let yo = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yo + e * 400;
    let doy = doe - (365 * yo + yo / 4 - yo / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y as i32, m as u32, d as u32)
}

/// 员工里已经绑定登录账号的数量，和没绑定的数量。
///
/// `Employee.user_id` 是可选字段——员工档案和登录账号之间是软关联，不是
/// 每个员工都配了账号。考勤、请假、定位打卡这些记录全部按 `user_id`
/// 归属，不认 `employee_id`；一个员工没绑定账号，他的考勤/请假记录
/// 就永远没法在系统里跟这个员工对上，只能靠 `user_id` 本身识别。
#[pure_function::pure]
pub(crate) fn employee_binding_counts(
    employees: &[crate::spacetime_bindings::employee_type::Employee],
) -> (usize, usize) {
    let mut bound = 0;
    let mut unbound = 0;
    for employee in employees.iter().filter(|employee| !employee.is_deleted) {
        if employee.user_id.is_some() {
            bound += 1;
        } else {
            unbound += 1;
        }
    }
    (bound, unbound)
}

/// 一批带可选归属 id 的记录里，有多少条落在给定的有效 id 集合里，
/// 有多少条对不上（id 为空，或指向的目标不在集合中）。
///
/// 最初为考勤/请假按 `user_id` 归属而写，但它对字段没有任何假设——
/// 数据地图用同一个函数核对工资→租赁客户、维护设备→园区等归属，
/// 所以名字不再提 user_id。
#[pure_function::pure]
pub(crate) fn linked_id_counts(
    ids: impl Iterator<Item = Option<u64>>,
    active_ids: &std::collections::BTreeSet<u64>,
) -> (usize, usize) {
    let mut linked = 0;
    let mut unlinked = 0;
    for id in ids {
        match id {
            Some(id) if active_ids.contains(&id) => linked += 1,
            _ => unlinked += 1,
        }
    }
    (linked, unlinked)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 人事日期与班次时间可往返() {
        assert_eq!(
            format_date(Some(parse_date("2026-07-13").unwrap())),
            "2026-07-13"
        );
        assert_eq!(parse_time("09:30"), Ok(Some(34200)));
    }

    fn employee(user_id: Option<u64>) -> crate::spacetime_bindings::employee_type::Employee {
        crate::spacetime_bindings::employee_type::Employee {
            employee_id: 1,
            customer_id: "public".into(),
            name: "员工".into(),
            gender: "".into(),
            phone: "".into(),
            user_id,
            age: None,
            id_number: None,
            address: None,
            education: None,
            department: None,
            hire_date: None,
            leave_date: None,
            remark: None,
            is_deleted: false,
            is_resigned: false,
            check_in_seconds: None,
            check_out_seconds: None,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 账号绑定统计区分已绑定和未绑定() {
        assert_eq!(
            employee_binding_counts(&[employee(Some(1)), employee(None), employee(Some(2))]),
            (2, 1)
        );
    }

    #[test]
    fn 已删除员工不参与账号绑定统计() {
        let mut deleted = employee(Some(1));
        deleted.is_deleted = true;
        assert_eq!(employee_binding_counts(&[deleted]), (0, 0));
    }

    #[test]
    fn 按用户_id_归属区分关联在职员工和对不上的记录() {
        let bound = std::collections::BTreeSet::from([1_u64, 2]);
        let ids = vec![Some(1), Some(2), Some(99), None];
        assert_eq!(linked_id_counts(ids.into_iter(), &bound), (2, 2));
    }

    #[test]
    fn 打卡来源有中文名且旧记录算手机定位() {
        assert_eq!(punch_source_label(Some("face")), "人脸识别");
        assert_eq!(punch_source_label(Some("gps")), "手机定位");
        // 加这一列之前写的记录没有取值，而那时只有手机打卡一个入口。
        assert_eq!(punch_source_label(None), "手机定位");
        // 库里出现没见过的取值时不能显示空白，否则一行看上去像数据丢了。
        assert_eq!(punch_source_label(Some("unknown")), "手机定位");
    }
}
