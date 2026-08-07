//! 门禁时间、状态和园区名称格式化。

use spacetimedb_sdk::Timestamp;

use crate::{services::park_ref, spacetime_bindings::park_type::Park};

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600 * MICROS_PER_SECOND;

#[pure_function::pure]
pub(super) fn status_label(status: i8, car: bool) -> &'static str {
    if car {
        if status == 1 {
            "进入"
        } else {
            "离开"
        }
    } else if status == 0 {
        "进入"
    } else {
        "离开"
    }
}

/// 状态到徽章：在场实心，离场描边。
///
/// 车辆和访客的状态编码是反的（车辆 1 为进入，访客 0 为进入），所以统一
/// 通过 `status_label` 判断，不要各自比数字。
#[pure_function::pure]
pub(super) fn status_variant(status: i8, car: bool) -> crate::components::badge::BadgeVariant {
    use crate::components::badge::BadgeVariant;
    if status_label(status, car) == "进入" {
        BadgeVariant::Secondary
    } else {
        BadgeVariant::Outline
    }
}

#[pure_function::pure]
pub(super) fn park_name(parks: &[Park], park_id: u64) -> String {
    park_ref(park_id)
        .and_then(|id| parks.iter().find(|park| park.park_id == id))
        .map(|park| park.park_name.clone())
        .unwrap_or_else(|| "未指定园区".into())
}

#[pure_function::pure]
pub(super) fn format_datetime(value: Timestamp) -> String {
    let local = value.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let days = local.div_euclid(MICROS_PER_DAY);
    let seconds = local.rem_euclid(MICROS_PER_DAY) / MICROS_PER_SECOND;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds / 3600,
        (seconds % 3600) / 60
    )
}

#[pure_function::pure]
pub(super) fn datetime_input(value: Timestamp) -> String {
    format_datetime(value).replace(' ', "T")
}

#[pure_function::pure]
pub(super) fn parse_datetime(value: &str) -> Result<Timestamp, String> {
    let (date, time) = value.trim().split_once('T').ok_or("登记时间格式不正确")?;
    let mut date_parts = date.split('-');
    let year = date_parts
        .next()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or("登记时间格式不正确")?;
    let month = date_parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("登记时间格式不正确")?;
    let day = date_parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("登记时间格式不正确")?;
    if date_parts.next().is_some() {
        return Err("登记时间格式不正确".into());
    }
    let mut time_parts = time.split(':');
    let hour = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 24)
        .ok_or("登记时间格式不正确")?;
    let minute = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 60)
        .ok_or("登记时间格式不正确")?;
    let days = days_from_civil(year, month, day).ok_or("登记时间格式不正确")?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY + (hour * 3600 + minute * 60) * MICROS_PER_SECOND
            - CHINA_OFFSET_MICROS,
    ))
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
    {
        return None;
    }
    let adjusted = year - i32::from(month <= 2);
    let era = adjusted.div_euclid(400);
    let yoe = adjusted - era * 400;
    let shifted = month as i32 + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) as i64)
}
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let doe = shifted - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 门禁本地时间可以往返() {
        let value = parse_datetime("2026-07-14T09:30").unwrap();
        assert_eq!(datetime_input(value), "2026-07-14T09:30");
    }
    #[test]
    fn 车辆和访客状态编码兼容原系统() {
        assert_eq!(status_label(1, true), "进入");
        assert_eq!(status_label(0, false), "进入");
    }
}
