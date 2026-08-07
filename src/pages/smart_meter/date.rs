//! 智能水电表管理冻结日期的轻量计算工具。

pub fn shift_date(value: &str, offset: i64) -> String {
    parse_date(value)
        .map(|(year, month, day)| civil_from_days(days_from_civil(year, month, day) + offset))
        .map(|(year, month, day)| format!("{year:04}-{month:02}-{day:02}"))
        .unwrap_or_else(today)
}

pub fn display_date(value: &str) -> String {
    parse_date(value)
        .map(|(year, month, day)| format!("{year}年{month}月{day}日"))
        .unwrap_or_else(|| value.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn today() -> String {
    let now = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date()
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn today() -> String {
    let unix_days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86_400;
    let (year, month, day) = civil_from_days(unix_days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// 当月的起止日期，都是 `YYYY-MM-DD`。
pub fn current_month_range() -> (String, String) {
    let today = today();
    month_range_of(&today).unwrap_or((today.clone(), today))
}

/// 给定日期所在自然月的起止日期。
///
/// 月末不查表：取下个月 1 号往前推一天，闰年和大小月都不必特判。
fn month_range_of(value: &str) -> Option<(String, String)> {
    let (year, month, _) = parse_date(value)?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let (last_year, last_month, last_day) =
        civil_from_days(days_from_civil(next_year, next_month, 1) - 1);
    Some((
        format!("{year:04}-{month:02}-01"),
        format!("{last_year:04}-{last_month:02}-{last_day:02}"),
    ))
}

/// 上一个月的同一天；本月这一天在上月不存在时退到上月最后一天。
///
/// 抄表按月对账，上期读数默认取上月同日——不能用「减 30 天」凑：3 月 31 日
/// 减 30 天落在 3 月 1 日，还在本月里，取出来的读数是同一个账期的。
pub fn previous_month_same_day(value: &str) -> String {
    let Some((year, month, day)) = parse_date(value) else {
        return value.to_string();
    };
    let (prev_year, prev_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    // 上月最后一天：本月 1 号往前一天。
    let (_, _, last_day) = civil_from_days(days_from_civil(year, month, 1) - 1);
    let day = day.min(last_day);
    format!("{prev_year:04}-{prev_month:02}-{day:02}")
}

fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    (parts.next().is_none() && (1..=12).contains(&month) && (1..=31).contains(&day))
        .then_some((year, month, day))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i32::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let shifted_days = days + 719_468;
    let era = shifted_days.div_euclid(146_097);
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = month_position + if month_position < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 冻结日期可以跨月切换() {
        assert_eq!(shift_date("2026-07-01", -1), "2026-06-30");
        assert_eq!(shift_date("2024-02-28", 1), "2024-02-29");
    }

    #[test]
    fn 自然月区间覆盖大小月与闰月() {
        assert_eq!(
            month_range_of("2026-07-24"),
            Some(("2026-07-01".into(), "2026-07-31".into()))
        );
        // 小月
        assert_eq!(
            month_range_of("2026-06-01"),
            Some(("2026-06-01".into(), "2026-06-30".into()))
        );
        // 闰年二月
        assert_eq!(
            month_range_of("2024-02-15"),
            Some(("2024-02-01".into(), "2024-02-29".into()))
        );
        // 平年二月
        assert_eq!(
            month_range_of("2026-02-15"),
            Some(("2026-02-01".into(), "2026-02-28".into()))
        );
        // 跨年：12 月要落到次年 1 月再回退
        assert_eq!(
            month_range_of("2026-12-09"),
            Some(("2026-12-01".into(), "2026-12-31".into()))
        );
    }

    #[test]
    fn 非法日期没有自然月区间() {
        assert_eq!(month_range_of(""), None);
        assert_eq!(month_range_of("2026-13-01"), None);
    }

    #[test]
    fn 上月同日在大小月与闰月都落得下() {
        assert_eq!(previous_month_same_day("2026-07-24"), "2026-06-24");
        // 3 月 31 日的上月同日不存在，退到 2 月最后一天。
        assert_eq!(previous_month_same_day("2026-03-31"), "2026-02-28");
        assert_eq!(previous_month_same_day("2024-03-31"), "2024-02-29");
        // 跨年
        assert_eq!(previous_month_same_day("2026-01-15"), "2025-12-15");
        // 非法输入原样返回，交给调用方处理
        assert_eq!(previous_month_same_day("x"), "x");
    }
}
