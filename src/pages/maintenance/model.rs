//! 维护页面共用的时间、地点与状态格式化。

use spacetimedb_sdk::Timestamp;

use crate::spacetime_bindings::{
    elevator_inspection_type::ElevatorInspection,
    firefighting_inspection_type::FirefightingInspection, factory_type::Factory, park_type::Park,
    transformer_inspection_type::TransformerInspection,
};

/// 设施巡检权限码，与服务端 `CODE_MAINTENANCE_INSPECT` 保持一致。
pub(super) const MAINTENANCE_INSPECT_CODE: &str = "maintenance:inspect";

/// 消防设施类型，与服务端 `FACILITY_TYPES` 保持一致。
pub(super) const FIREFIGHTING_TYPES: [&str; 5] =
    ["灭火器", "消防栓", "消防出口", "应急照明", "其他"];

const MICROS_PER_SECOND: i64 = 1_000_000;
const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600 * MICROS_PER_SECOND;

#[cfg(target_arch = "wasm32")]
pub(super) fn now_timestamp() -> Timestamp {
    // 浏览器 WASM 不支持 std::time::SystemTime，必须从 JavaScript 运行时读取时间。
    let micros = (js_sys::Date::now() * 1_000.0).min(i64::MAX as f64) as i64;
    Timestamp::from_micros_since_unix_epoch(micros)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn now_timestamp() -> Timestamp {
    // 服务端和原生测试继续使用系统时钟，保证 SSR 与测试环境无需依赖浏览器 API。
    let micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_micros().min(i64::MAX as u128) as i64)
        .unwrap_or_default();
    Timestamp::from_micros_since_unix_epoch(micros)
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
    let (date, time) = value.trim().split_once('T').ok_or("检查时间格式不正确")?;
    let mut date_parts = date.split('-');
    let year = date_parts
        .next()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or("检查时间格式不正确")?;
    let month = date_parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("检查时间格式不正确")?;
    let day = date_parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("检查时间格式不正确")?;
    let mut time_parts = time.split(':');
    let hour = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 24)
        .ok_or("检查时间格式不正确")?;
    let minute = time_parts
        .next()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v < 60)
        .ok_or("检查时间格式不正确")?;
    let days = days_from_civil(year, month, day).ok_or("检查时间格式不正确")?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY + (hour * 3600 + minute * 60) * MICROS_PER_SECOND
            - CHINA_OFFSET_MICROS,
    ))
}

#[pure_function::pure]
pub(super) fn park_name(parks: &[Park], park_id: u64) -> String {
    parks
        .iter()
        .find(|row| row.park_id == park_id)
        .map(|row| row.park_name.clone())
        .unwrap_or_else(|| format!("园区 #{park_id}"))
}

#[pure_function::pure]
pub(super) fn factory_name(factories: &[Factory], factory_id: Option<u64>) -> String {
    factory_id
        .and_then(|id| factories.iter().find(|row| row.factory_id == id))
        .map(|row| row.factory_name.clone())
        .unwrap_or_else(|| "未指定厂房".into())
}

/// 状态到徽章。四类维护业务共用一张表，状态文案来自不同的枚举。
#[pure_function::pure]
pub(super) fn status_variant(status: &str) -> crate::components::badge::BadgeVariant {
    use crate::components::badge::BadgeVariant;
    match status {
        "正常" | "已完成" => BadgeVariant::Secondary,
        "异常" | "紧急" | "已取消" => BadgeVariant::Destructive,
        _ => BadgeVariant::Outline,
    }
}

/// 容量展示：`1100` → `11.00 kW`。0 视为未填写。
#[pure_function::pure]
pub(super) fn format_capacity_kw(centi_kw: u64) -> String {
    if centi_kw == 0 {
        return "未填写".into();
    }
    format!("{}.{:02} kW", centi_kw / 100, centi_kw % 100)
}

/// 容量回填到输入框：`1100` → `11.00`，0 → 空串。
#[pure_function::pure]
pub(super) fn capacity_input(centi_kw: u64) -> String {
    if centi_kw == 0 {
        return String::new();
    }
    format!("{}.{:02}", centi_kw / 100, centi_kw % 100)
}

/// 解析千瓦输入为乘一百的整数；空串视为未填写（0）。
#[pure_function::pure]
pub(super) fn parse_capacity_kw(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() && (0.0..=1_000_000.0).contains(&v) => {
            Ok((v * 100.0).round() as u64)
        }
        _ => Err("容量格式不正确，请输入非负数字（千瓦）".into()),
    }
}

/// 某台设备最近的一次巡检。
#[pure_function::pure]
pub(crate) fn latest_inspection(
    inspections: &[TransformerInspection],
    asset_id: u64,
) -> Option<TransformerInspection> {
    inspections
        .iter()
        .filter(|row| row.asset_id == asset_id)
        .max_by_key(|row| (row.check_time, row.inspection_id))
        .cloned()
}

/// 某台电梯最近的一次巡检。
#[pure_function::pure]
pub(crate) fn latest_elevator_inspection(
    inspections: &[ElevatorInspection],
    asset_id: u64,
) -> Option<ElevatorInspection> {
    inspections
        .iter()
        .filter(|row| row.asset_id == asset_id)
        .max_by_key(|row| (row.check_time, row.inspection_id))
        .cloned()
}

/// 某个消防设施最近的一次巡检。
#[pure_function::pure]
pub(crate) fn latest_firefighting_inspection(
    inspections: &[FirefightingInspection],
    asset_id: u64,
) -> Option<FirefightingInspection> {
    inspections
        .iter()
        .filter(|row| row.asset_id == asset_id)
        .max_by_key(|row| (row.check_time, row.inspection_id))
        .cloned()
}

/// 今天的日期（中国时区）`YYYY-MM-DD`。读时钟，因此按约定以 today 命名。
pub(crate) fn today_date() -> String {
    format_datetime(now_timestamp())[..10].to_string()
}

/// 有效期距今还有几天：负数表示已过期。日期非法时返回 `None`（不提醒）。
#[pure_function::pure]
pub(crate) fn expiry_days_left(expiry_on: &str, today: &str) -> Option<i64> {
    let expiry = civil_days_of(expiry_on)?;
    let now = civil_days_of(today)?;
    Some(expiry - now)
}

fn civil_days_of(date: &str) -> Option<i64> {
    let mut parts = date.trim().split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    days_from_civil(year, month, day)
}

/// 承重展示：`200000` → `2000.00 kg`。0 视为未填写。
#[pure_function::pure]
pub(super) fn format_load_kg(centi_kg: u64) -> String {
    if centi_kg == 0 {
        return "未填写".into();
    }
    format!("{}.{:02} kg", centi_kg / 100, centi_kg % 100)
}

/// 承重回填到输入框：`200000` → `2000.00`，0 → 空串。
#[pure_function::pure]
pub(super) fn load_kg_input(centi_kg: u64) -> String {
    if centi_kg == 0 {
        return String::new();
    }
    format!("{}.{:02}", centi_kg / 100, centi_kg % 100)
}

/// 解析千克输入为乘一百的整数；空串视为未填写（0）。
#[pure_function::pure]
pub(super) fn parse_load_kg(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    match trimmed.parse::<f64>() {
        Ok(v) if v.is_finite() && (0.0..=10_000_000.0).contains(&v) => {
            Ok((v * 100.0).round() as u64)
        }
        _ => Err("额定承重格式不正确，请输入非负数字（千克）".into()),
    }
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
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let mut year = (yoe + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let day = (doy - (153 * mp + 2).div_euclid(5) + 1) as u32;
    let month = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    year += i32::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 维护检查时间可以按中国时区往返() {
        let value = parse_datetime("2026-07-15T09:30").expect("时间应可解析");
        assert_eq!(datetime_input(value), "2026-07-15T09:30");
    }

    #[test]
    fn 非法维护检查时间会被拒绝() {
        assert!(parse_datetime("2026-02-30T09:30").is_err());
        assert!(parse_datetime("2026-07-15 09:30").is_err());
    }

    #[test]
    fn 容量在输入与展示之间可以往返() {
        assert_eq!(parse_capacity_kw("11.00").expect("应可解析"), 1100);
        assert_eq!(parse_capacity_kw("  "), Ok(0));
        assert_eq!(format_capacity_kw(1100), "11.00 kW");
        assert_eq!(format_capacity_kw(0), "未填写");
        assert_eq!(capacity_input(1100), "11.00");
        assert_eq!(capacity_input(0), "");
        assert!(parse_capacity_kw("-1").is_err());
        assert!(parse_capacity_kw("abc").is_err());
    }

    #[test]
    fn 有效期剩余天数按日历计算() {
        assert_eq!(expiry_days_left("2026-08-05", "2026-08-04"), Some(1));
        assert_eq!(expiry_days_left("2026-08-04", "2026-08-04"), Some(0));
        assert_eq!(expiry_days_left("2026-08-01", "2026-08-04"), Some(-3));
        // 跨月与非法输入。
        assert_eq!(expiry_days_left("2026-09-03", "2026-08-04"), Some(30));
        assert_eq!(expiry_days_left("2026-13-01", "2026-08-04"), None);
        assert_eq!(expiry_days_left("", "2026-08-04"), None);
    }

    #[test]
    fn 承重在输入与展示之间可以往返() {
        assert_eq!(parse_load_kg("2000.00").expect("应可解析"), 200_000);
        assert_eq!(parse_load_kg(""), Ok(0));
        assert_eq!(format_load_kg(200_000), "2000.00 kg");
        assert_eq!(format_load_kg(0), "未填写");
        assert_eq!(load_kg_input(200_000), "2000.00");
        assert!(parse_load_kg("-5").is_err());
    }
}
