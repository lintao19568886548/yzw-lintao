//! 报销金额、日期、状态、优先级和权限的纯计算模型。

use spacetimedb_sdk::Timestamp;

use crate::spacetime_bindings::{reimbursement_type::Reimbursement, role_type::Role};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReimbursementStatus {
    Pending,
    Approved,
    Rejected,
}

impl ReimbursementStatus {
    #[pure_function::pure]
    pub(super) fn from_value(value: i8) -> Self {
        match value {
            1 => Self::Approved,
            2 => Self::Rejected,
            _ => Self::Pending,
        }
    }

    #[pure_function::pure]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pending => "待审核",
            Self::Approved => "已通过",
            Self::Rejected => "已驳回",
        }
    }

    /// 状态到徽章：已通过实心，已驳回危险色，待审核描边。
    #[pure_function::pure]
    pub(super) fn badge_variant(self) -> crate::components::badge::BadgeVariant {
        use crate::components::badge::BadgeVariant;
        match self {
            Self::Approved => BadgeVariant::Secondary,
            Self::Rejected => BadgeVariant::Destructive,
            Self::Pending => BadgeVariant::Outline,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AuditPriority {
    Normal,
    Warning,
    Urgent,
    Done,
}

impl AuditPriority {
    /// 优先级到徽章：紧急用危险色，其余描边——已处理的不需要再吸引注意。
    #[pure_function::pure]
    pub(super) fn badge_variant(self) -> crate::components::badge::BadgeVariant {
        use crate::components::badge::BadgeVariant;
        match self {
            Self::Urgent => BadgeVariant::Destructive,
            Self::Warning => BadgeVariant::Secondary,
            Self::Normal | Self::Done => BadgeVariant::Outline,
        }
    }

    #[pure_function::pure]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Normal => "常规审核",
            Self::Warning => "重点关注",
            Self::Urgent => "紧急处理",
            Self::Done => "流程完成",
        }
    }
}

#[pure_function::pure]
pub(super) fn waiting_days(row: &Reimbursement, now: Timestamp) -> i64 {
    if row.status != 0 {
        return 0;
    }
    (now.to_micros_since_unix_epoch() - row.created_at.to_micros_since_unix_epoch())
        .div_euclid(MICROS_PER_DAY)
        .max(0)
}

/// 与原系统一致：等待 7 天或 10 万元为紧急，等待 3 天或 1 万元为关注。
#[pure_function::pure]
pub(super) fn audit_priority(row: &Reimbursement, now: Timestamp) -> AuditPriority {
    if row.status != 0 {
        return AuditPriority::Done;
    }
    let days = waiting_days(row, now);
    if days >= 7 || row.amount_cents >= 10_000_000 {
        AuditPriority::Urgent
    } else if days >= 3 || row.amount_cents >= 1_000_000 {
        AuditPriority::Warning
    } else {
        AuditPriority::Normal
    }
}

#[pure_function::pure]
pub(super) fn priority_reason(row: &Reimbursement, now: Timestamp) -> String {
    let days = waiting_days(row, now);
    if row.status != 0 {
        return "审核流程已完成".into();
    }
    if days > 0 {
        format!("已等待 {days} 天")
    } else if row.amount_cents >= 10_000_000 {
        "大额报销".into()
    } else if row.amount_cents >= 1_000_000 {
        "金额较高".into()
    } else {
        "今日提交".into()
    }
}

#[pure_function::pure]
pub(super) fn can_audit(roles: &[Role]) -> bool {
    roles.iter().any(|role| {
        role.status == 1
            && ((role.name == "Super" && role.scope == "system")
                || role.reimbursement_auth.unwrap_or(0) > 0)
    })
}

#[pure_function::pure]
pub(super) fn audit_limit(roles: &[Role]) -> Option<i64> {
    let audit_roles = roles
        .iter()
        .filter(|role| {
            role.status == 1
                && ((role.name == "Super" && role.scope == "system")
                    || role.reimbursement_auth.unwrap_or(0) > 0)
        })
        .collect::<Vec<_>>();
    if audit_roles
        .iter()
        .any(|role| role.name == "Super" || role.rates.is_none_or(|value| value < 0))
    {
        return None;
    }
    audit_roles
        .iter()
        .filter_map(|role| role.rates)
        .max()
        .map(|yuan| i64::from(yuan) * 100)
}

#[pure_function::pure]
pub(super) fn format_money(cents: i64) -> String {
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

#[pure_function::pure]
pub(super) fn parse_amount(value: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("请输入报销金额".into());
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or("报销金额格式不正确")?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || fraction.len() > 2
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("报销金额最多保留两位小数".into());
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().unwrap_or_default() * 10,
        _ => fraction.parse::<i64>().unwrap_or_default(),
    };
    let cents = whole
        .checked_mul(100)
        .and_then(|value| value.checked_add(fraction))
        .ok_or("报销金额过大")?;
    (cents > 0)
        .then_some(cents)
        .ok_or("报销金额必须大于0".into())
}

pub(super) fn today_timestamp() -> Timestamp {
    parse_date(&crate::pages::smart_meter::date::today())
        .unwrap_or_else(|_| Timestamp::from_micros_since_unix_epoch(0))
}

#[pure_function::pure]
pub(super) fn parse_date(value: &str) -> Result<Timestamp, String> {
    let mut parts = value.trim().split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    if parts.next().is_some() {
        return Err("日期格式不正确".into());
    }
    let days = year
        .zip(month)
        .zip(day)
        .and_then(|((year, month), day)| days_from_civil(year, month, day))
        .ok_or("请选择正确的报销日期")?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    ))
}

#[pure_function::pure]
pub(super) fn format_date(timestamp: Timestamp) -> String {
    let days =
        (timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS).div_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn month_days(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || day == 0
        || day > month_days(year, month)
    {
        return None;
    }
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    y -= i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    Some(era * 146_097 + (yoe * 365 + yoe / 4 - yoe / 100 + (153 * mp + 2) / 5 + d - 1) - 719_468)
}
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn 报销金额和日期可以严格解析() {
        assert_eq!(parse_amount("1000.5"), Ok(100_050));
        assert!(parse_amount("1.234").is_err());
        assert_eq!(format_date(parse_date("2026-07-13").unwrap()), "2026-07-13");
    }
}
