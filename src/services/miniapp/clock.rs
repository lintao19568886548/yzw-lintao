use std::time::{SystemTime, UNIX_EPOCH};

use time::{Date, Duration, Month, OffsetDateTime, UtcOffset};

pub const BUSINESS_TIMEZONE: &str = "Asia/Shanghai";

pub trait Clock: Send + Sync {
    fn now_epoch_seconds(&self) -> u64;
    fn today_china(&self) -> Date;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_epoch_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn today_china(&self) -> Date {
        china_date_from_epoch(self.now_epoch_seconds()).unwrap_or(Date::MIN)
    }
}

pub fn china_date_from_epoch(epoch_seconds: u64) -> Result<Date, String> {
    let timestamp = i64::try_from(epoch_seconds).map_err(|_| "服务器时间超出支持范围")?;
    let china_offset = UtcOffset::from_hms(8, 0, 0).map_err(|_| "中国标准时偏移配置无效")?;
    OffsetDateTime::from_unix_timestamp(timestamp)
        .map(|value| value.to_offset(china_offset).date())
        .map_err(|_| "服务器时间超出支持范围".into())
}

pub fn parse_iso_date(value: &str) -> Result<Date, String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err("日期必须使用YYYY-MM-DD格式".into());
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return Err("日期必须使用YYYY-MM-DD格式".into());
    }
    let year = value[0..4].parse::<i32>().map_err(|_| "日期年份无效")?;
    let month_number = value[5..7].parse::<u8>().map_err(|_| "日期月份无效")?;
    let day = value[8..10].parse::<u8>().map_err(|_| "日期日无效")?;
    let month = Month::try_from(month_number).map_err(|_| "日期月份无效")?;
    Date::from_calendar_date(year, month, day).map_err(|_| "日期不是有效日历日期".into())
}

pub fn move_in_deadline(value: &str, reference_date: Date) -> Result<Date, String> {
    match value {
        "immediate" => Ok(reference_date),
        "within_30_days" => reference_date
            .checked_add(Duration::days(30))
            .ok_or_else(|| "30天入驻日期超出支持范围".into()),
        "within_90_days" => reference_date
            .checked_add(Duration::days(90))
            .ok_or_else(|| "90天入驻日期超出支持范围".into()),
        date => parse_iso_date(date),
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct FixedClock {
    epoch_seconds: std::sync::atomic::AtomicU64,
    date: std::sync::RwLock<Date>,
}

#[cfg(test)]
impl FixedClock {
    pub fn new(epoch_seconds: u64, date: Date) -> Self {
        Self {
            epoch_seconds: std::sync::atomic::AtomicU64::new(epoch_seconds),
            date: std::sync::RwLock::new(date),
        }
    }

    pub fn set_epoch_seconds(&self, value: u64) {
        self.epoch_seconds
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_date(&self, value: Date) {
        *self.date.write().expect("fixed clock date lock") = value;
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> u64 {
        self.epoch_seconds
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn today_china(&self) -> Date {
        *self.date.read().expect("fixed clock date lock")
    }
}
