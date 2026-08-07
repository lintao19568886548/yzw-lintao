//! 财务流水金额与日期转换，以及账单/报销与流水之间的关联统计。

use std::collections::BTreeSet;

use spacetimedb_sdk::Timestamp;

use crate::spacetime_bindings::{
    amount_bill_type::AmountBill, finance_type::Finance, reimbursement_type::Reimbursement,
};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;

#[pure_function::pure]
pub(super) fn format_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let value = cents.unsigned_abs();
    let whole = value / 100;
    let grouped = whole
        .to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(",");
    format!("{sign}¥{grouped}.{:02}", value % 100)
}

#[pure_function::pure]
pub(super) fn money_input(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.unsigned_abs() % 100)
}

#[pure_function::pure]
pub(super) fn parse_money(value: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("请输入流水金额".into());
    }
    let amount = value
        .parse::<f64>()
        .map_err(|_| "流水金额格式不正确".to_string())?;
    if !amount.is_finite() || amount < 0.0 {
        return Err("流水金额不能为负数".into());
    }
    Ok((amount * 100.0).round() as i64)
}

#[pure_function::pure]
pub(super) fn format_date(timestamp: Timestamp) -> String {
    let days =
        (timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS).div_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

pub(super) fn today() -> String {
    crate::pages::smart_meter::date::today()
}

#[pure_function::pure]
pub(super) fn parse_date(value: &str) -> Result<Timestamp, String> {
    let mut parts = value.trim().split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    if parts.next().is_some() {
        return Err("交易日期格式不正确".into());
    }
    let days = year
        .zip(month)
        .zip(day)
        .and_then(|((year, month), day)| days_from_civil(year, month, day))
        .ok_or_else(|| "请选择正确的交易日期".to_string())?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    ))
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return None;
    }
    let year = i64::from(year) - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 }.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

fn active_finance_ids(finances: &[Finance]) -> BTreeSet<u64> {
    finances
        .iter()
        .filter(|finance| !finance.is_deleted)
        .map(|finance| finance.finance_id)
        .collect()
}

/// 账单里 `finance_id` 真的指向一条未删除财务流水的数量，和指不上的数量。
///
/// `AmountBill.finance_id` 是必填外键——原本设计上每张账单都会同时生成
/// 一条流水；指不上通常意味着那条流水后来被删除了。
#[pure_function::pure]
pub(crate) fn bill_finance_link_counts(
    bills: &[AmountBill],
    finances: &[Finance],
) -> (usize, usize) {
    let active = active_finance_ids(finances);
    let mut linked = 0;
    let mut orphaned = 0;
    for bill in bills {
        if active.contains(&bill.finance_id) {
            linked += 1;
        } else {
            orphaned += 1;
        }
    }
    (linked, orphaned)
}

/// 水电明细里 `bill_id` 真的指向一张已存在账单的数量，和指不上的数量。
///
/// 账单表没有软删除字段，这里的"指不上"只可能是数据本身的残留，不存在
/// "父账单被删除"这一种情况——跟楼层/厂房那条链路的失联原因不一样。
#[pure_function::pure]
pub(crate) fn utility_bill_link_counts(bill_ids: &[u64], bills: &[AmountBill]) -> (usize, usize) {
    let known = bills
        .iter()
        .map(|bill| bill.bill_id)
        .collect::<BTreeSet<_>>();
    let mut linked = 0;
    let mut orphaned = 0;
    for bill_id in bill_ids {
        if known.contains(bill_id) {
            linked += 1;
        } else {
            orphaned += 1;
        }
    }
    (linked, orphaned)
}

/// 未删除报销里，`finance_id` 已经指向一条未删除流水的数量，和还没有的
/// 数量。
///
/// 这里是可选外键——报销从提交到最终生成付款流水中间有审批流程，
/// 还没批下来的报销本来就不该有关联的流水，不是数据缺口。
#[pure_function::pure]
pub(crate) fn reimbursement_finance_link_counts(
    reimbursements: &[Reimbursement],
    finances: &[Finance],
) -> (usize, usize) {
    let active = active_finance_ids(finances);
    let mut linked = 0;
    let mut pending = 0;
    for reimbursement in reimbursements.iter().filter(|row| !row.is_deleted) {
        match reimbursement.finance_id {
            Some(finance_id) if active.contains(&finance_id) => linked += 1,
            _ => pending += 1,
        }
    }
    (linked, pending)
}

#[cfg(test)]
mod link_tests {
    use super::*;
    use crate::services::NO_PARK;

    fn finance(finance_id: u64, is_deleted: bool) -> Finance {
        Finance {
            finance_id,
            customer_id: "public".into(),
            bill_name: "流水".into(),
            bill_category: "".into(),
            amount_cents: 0,
            transaction_type: "expense".into(),
            transaction_time: Timestamp::UNIX_EPOCH,
            remark: None,
            park_id: 0,
            status: 1,
            is_deleted,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn bill(bill_id: u64, finance_id: u64) -> AmountBill {
        AmountBill {
            bill_id,
            customer_id: "public".into(),
            project_name: "".into(),
            tenant_name: None,
            public_bank_account: None,
            private_bank_account: None,
            ele_fee_cents: 0,
            extra_ele_fee_cents: 0,
            basic_ele_fee_cents: 0,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            water_fee_cents: 0,
            receive_fee_cents: 0,
            factory_rent_cents: 0,
            management_fee_cents: 0,
            invoice_tax_cents: 0,
            total_fee_cents: 0,
            service_fee_cents: 0,
            garbage_fee_cents: 0,
            receipt_amount_cents: 0,
            penalty_fee_cents: None,
            service_rate_basis_points: None,
            garbage_rate_basis_points: None,
            penalty_rate_basis_points: None,
            extra_ele_rate_basis_points: None,
            penalty_item: None,
            extra_ele_item: None,
            ele_item: None,
            water_item: None,
            extra_project_item: None,
            tax_rate_json: None,
            remark: None,
            receipt_time: None,
            finance_id,
            tenant_id: 0,
            park_id: 0,
            created_at: Timestamp::UNIX_EPOCH,
            carryover_fee_cents: 0,
            carryover_item: None,
            updated_at: None,
        }
    }

    fn reimbursement(finance_id: Option<u64>, is_deleted: bool) -> Reimbursement {
        Reimbursement {
            id: 1,
            customer_id: "public".into(),
            purpose: "".into(),
            amount_cents: 0,
            payee: "".into(),
            reimbursement_date: Timestamp::UNIX_EPOCH,
            department: None,
            username: None,
            remark: None,
            status: 1,
            is_deleted,
            user_id: None,
            park_id: NO_PARK,
            audit_opinion: None,
            claimant: None,
            finance_id,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 账单归属统计区分已关联流水和流水已删除或不存在() {
        let finances = [finance(1, false), finance(2, true)];
        let bills = [bill(10, 1), bill(11, 2), bill(12, 999)];
        assert_eq!(bill_finance_link_counts(&bills, &finances), (1, 2));
    }

    #[test]
    fn 水电明细归属统计区分账单是否存在() {
        let bills = [bill(10, 1)];
        assert_eq!(utility_bill_link_counts(&[10, 10, 999], &bills), (2, 1));
    }

    #[test]
    fn 报销与流水关联区分已生成和待生成() {
        let finances = [finance(1, false)];
        let rows = [
            reimbursement(Some(1), false),   // 已生成有效流水
            reimbursement(Some(999), false), // 流水不存在或已删除
            reimbursement(None, false),      // 还没生成流水
        ];
        assert_eq!(reimbursement_finance_link_counts(&rows, &finances), (1, 2));
    }

    #[test]
    fn 已删除的报销不参与关联统计() {
        let mut deleted = reimbursement(Some(1), true);
        deleted.is_deleted = true;
        assert_eq!(reimbursement_finance_link_counts(&[deleted], &[]), (0, 0));
    }
}
