//! 工资金额与中国时区日期格式化。

use spacetimedb_sdk::Timestamp;

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;

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
    let mut year = i64::from(year);
    let month = i64::from(month);
    let day = i64::from(day);
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

pub fn parse_date(value: &str) -> Result<Timestamp, String> {
    let mut parts = value.split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    if parts.next().is_some() {
        return Err("发放日期格式不正确".into());
    }
    let days = year
        .zip(month)
        .zip(day)
        .and_then(|((year, month), day)| days_from_civil(year, month, day))
        .ok_or_else(|| "请选择正确的发放日期".to_string())?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    ))
}

pub fn format_date(timestamp: Option<Timestamp>) -> String {
    let Some(timestamp) = timestamp else {
        return "--".into();
    };
    let local_micros = timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let (year, month, day) = civil_from_days(local_micros.div_euclid(MICROS_PER_DAY));
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn parse_amount_to_cents(value: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("请输入工资金额".into());
    }
    if value.starts_with('-') {
        return Err("工资金额不能为负数".into());
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(|| "工资金额格式不正确".to_string())?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() || fraction.len() > 2 || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return Err("工资金额最多保留两位小数".into());
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().unwrap_or_default() * 10,
        _ => fraction.parse::<i64>().unwrap_or_default(),
    };
    whole
        .checked_mul(100)
        .and_then(|value| value.checked_add(fraction))
        .ok_or_else(|| "工资金额过大".to_string())
}

pub fn cents_input_value(cents: Option<i64>) -> String {
    cents
        .map(|value| format!("{}.{:02}", value / 100, value.unsigned_abs() % 100))
        .unwrap_or_default()
}

pub fn format_money(cents: Option<i64>) -> String {
    let Some(cents) = cents else {
        return "--".into();
    };
    let sign = if cents < 0 { "-" } else { "" };
    let absolute = cents.unsigned_abs();
    let whole = absolute / 100;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    format!("{sign}¥{grouped}.{:02}", absolute % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 工资金额严格换算为分() {
        assert_eq!(parse_amount_to_cents("1234.5"), Ok(123_450));
        assert_eq!(format_money(Some(1_234_567)), "¥12,345.67");
        assert!(parse_amount_to_cents("12.345").is_err());
    }

    #[test]
    fn 中国时区工资日期可以往返() {
        let timestamp = parse_date("2026-07-13").expect("日期应有效");
        assert_eq!(format_date(Some(timestamp)), "2026-07-13");
        assert!(parse_date("2026-02-30").is_err());
    }
}
