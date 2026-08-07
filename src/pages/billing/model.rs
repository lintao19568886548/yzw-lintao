//! 账单金额、日期、收款状态和账期计算。

use std::collections::BTreeMap;

use spacetimedb_sdk::Timestamp;

use crate::spacetime_bindings::{
    amount_bill_type::AmountBill, bill_collection_confirmation_type::BillCollectionConfirmation,
    carryover_batch_type::CarryoverBatch, carryover_item_type::CarryoverItem,
};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CollectionStatus {
    Unpaid,
    Partial,
    Paid,
    Overpaid,
}

impl CollectionStatus {
    #[pure_function::pure]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Unpaid => "未收款",
            Self::Partial => "部分收款",
            Self::Paid => "已收款",
            Self::Overpaid => "多收",
        }
    }

    #[pure_function::pure]
    pub(super) fn value(self) -> &'static str {
        match self {
            Self::Unpaid => "unpaid",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Overpaid => "overpaid",
        }
    }

    /// 收款状态到徽章：已收实心，未收和多收用危险色（都需要跟进），部分收款描边。
    #[pure_function::pure]
    pub(super) fn badge_variant(self) -> crate::components::badge::BadgeVariant {
        use crate::components::badge::BadgeVariant;
        match self {
            Self::Paid => BadgeVariant::Secondary,
            Self::Unpaid | Self::Overpaid => BadgeVariant::Destructive,
            Self::Partial => BadgeVariant::Outline,
        }
    }
}

/// 确认类型：已收齐。与服务端 `CONFIRM_TYPE_COLLECTED` 保持一致。
pub(super) const CONFIRM_TYPE_COLLECTED: &str = "collected";
/// 确认类型：差额。与服务端 `CONFIRM_TYPE_SHORTFALL` 保持一致。
pub(super) const CONFIRM_TYPE_SHORTFALL: &str = "shortfall";

/// 收缴对账状态。
///
/// **镜像**服务端 `reducers/finance/billing/reconciliation.rs` 的状态机——两边的
/// 清单必须保持一致，服务端是权威：这里只决定徽章与按钮，骗得过按钮骗不过
/// Reducer。改任何一边前先对照另一边。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReconciliationState {
    /// 待收款：还没有任何收款动作。
    AwaitingReceipt,
    /// 待确认收齐：实收已达应收，只差经理点一下——「钱到了却没确认」的提醒态。
    AwaitingCollectedConfirm,
    /// 待确认差额：实收不足，经理还没有知悉。
    AwaitingShortfallConfirm,
    /// 差额催交中：经理已确认差额，二次催交进行中。
    ShortfallChasing,
    /// 已闭环：经理已确认收齐，终态。
    Closed,
}

impl ReconciliationState {
    #[pure_function::pure]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::AwaitingReceipt => "待收款",
            Self::AwaitingCollectedConfirm => "待确认收齐",
            Self::AwaitingShortfallConfirm => "待确认差额",
            Self::ShortfallChasing => "差额催交中",
            Self::Closed => "已闭环",
        }
    }

    /// 徽章样式。「待确认收齐」用危险色高亮——钱到了、系统知道、只差一次点击，
    /// 停在这里的账单会在账期截止时被结转成一笔并不存在的欠款（文档 §4.1）。
    #[pure_function::pure]
    pub(super) fn badge_variant(self) -> crate::components::badge::BadgeVariant {
        use crate::components::badge::BadgeVariant;
        match self {
            Self::AwaitingCollectedConfirm => BadgeVariant::Destructive,
            Self::Closed => BadgeVariant::Secondary,
            Self::AwaitingReceipt | Self::AwaitingShortfallConfirm | Self::ShortfallChasing => {
                BadgeVariant::Outline
            }
        }
    }

    /// 「确认已收齐」按钮是否可用。允许状态与服务端 `ACTION_RULES` 一致。
    #[pure_function::pure]
    pub(super) fn can_confirm_collected(self) -> bool {
        self == Self::AwaitingCollectedConfirm
    }

    /// 「确认差额」按钮是否可用。允许状态与服务端 `ACTION_RULES` 一致。
    #[pure_function::pure]
    pub(super) fn can_confirm_shortfall(self) -> bool {
        matches!(self, Self::AwaitingShortfallConfirm | Self::ShortfallChasing)
    }
}

/// 状态判定：自上而下第一条命中即当前状态，顺序即优先级。
///
/// 与服务端 `STATE_RULES` 逐条对应：收齐确认压过一切（终态），实收达标压过
/// 差额确认（催缴期间补齐了就该走确认收齐），没有收款动作一律待收款——
/// 应收为 0 的历史账单不能一建出来就变成待确认。
#[pure_function::pure]
pub(super) fn reconciliation_state(
    bill: &AmountBill,
    has_collected_confirm: bool,
    has_shortfall_confirm: bool,
) -> ReconciliationState {
    let has_movement = bill.receipt_amount_cents > 0 || bill.receipt_time.is_some();
    if has_collected_confirm {
        ReconciliationState::Closed
    } else if has_movement && bill.receipt_amount_cents >= bill.total_fee_cents {
        ReconciliationState::AwaitingCollectedConfirm
    } else if has_shortfall_confirm {
        ReconciliationState::ShortfallChasing
    } else if has_movement {
        ReconciliationState::AwaitingShortfallConfirm
    } else {
        ReconciliationState::AwaitingReceipt
    }
}

/// 按账单聚合确认记录：`bill_id -> (有收齐确认, 有差额确认)`。
///
/// 表格逐行判状态,先聚合一次省得每行扫全量确认记录。
#[pure_function::pure]
pub(super) fn confirmation_flags(
    confirmations: &[BillCollectionConfirmation],
) -> BTreeMap<u64, (bool, bool)> {
    let mut flags = BTreeMap::<u64, (bool, bool)>::new();
    for row in confirmations {
        let entry = flags.entry(row.bill_id).or_default();
        entry.0 |= row.confirm_type == CONFIRM_TYPE_COLLECTED;
        entry.1 |= row.confirm_type == CONFIRM_TYPE_SHORTFALL;
    }
    flags
}


/// 批次状态：等经理核对。与服务端 `BATCH_STATUS_PENDING` 一致。
pub(super) const BATCH_PENDING: &str = "pending";

/// 处置取值，与服务端 `DISPOSITION_*` 一一对应。
pub(super) const DISPOSITION_CARRY: &str = "carry";
pub(super) const DISPOSITION_SKIP: &str = "skip";
pub(super) const DISPOSITION_COLLECTED: &str = "collected";

/// 处置的中文名与说明。
#[pure_function::pure]
pub(super) fn disposition_label(disposition: &str) -> (&'static str, &'static str) {
    match disposition {
        DISPOSITION_SKIP => ("本次不结转", "本期挂起，下期还会出现"),
        DISPOSITION_COLLECTED => ("剔除并确认收齐", "补一条收齐确认，账单闭环"),
        // 未知取值按默认处置显示，不能渲染成空白——空白会让人以为这行坏了。
        _ => ("结转到下期", "差额并入下一账期的应收"),
    }
}

/// 一张结转项要结转多少钱。**镜像**服务端 `carryover_amount_cents`。
///
/// 只有 `carry` 产生结转额：`skip` 是挂起、`collected` 是认定已收齐，
/// 两者都不该把钱滚到下个月。
#[pure_function::pure]
pub(super) fn carryover_amount_cents(item: &CarryoverItem) -> i64 {
    if item.disposition != DISPOSITION_CARRY {
        return 0;
    }
    item.receivable_cents
        .saturating_sub(item.received_cents)
        .max(0)
}

/// 批次合计：结转笔数与结转总额。
#[pure_function::pure]
pub(super) fn carryover_totals(items: &[CarryoverItem]) -> (usize, i64) {
    items.iter().fold((0usize, 0i64), |(count, sum), item| {
        let amount = carryover_amount_cents(item);
        if item.disposition == DISPOSITION_CARRY {
            (count + 1, sum.saturating_add(amount))
        } else {
            (count, sum)
        }
    })
}

/// 批次状态：经理已确认。与服务端 `BATCH_STATUS_CONFIRMED` 一致。
pub(super) const BATCH_CONFIRMED: &str = "confirmed";

/// 可并入新账单的一笔结转。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CarryoverOption {
    pub item_id: u64,
    /// 结转来源账期，如「2026-07」。
    pub period_label: String,
    /// 原账单项目名，让经理认得出这是哪一笔。
    pub project_name: String,
    pub shortfall_cents: i64,
}

/// 某租户当前可并入账单的结转项。**镜像**服务端 `take_consumable_items` 的准入条件。
///
/// 四个条件缺一不可：批次已确认、处置为结转、未被别的账单占用、来源账单属这个
/// 租户。前三条与服务端一致——这里只决定列表里显示什么，骗得过界面骗不过 Reducer。
///
/// **租户与项目名取自结转项自带的快照，不查 `amount_bill`**：账单页的账单走
/// 分页 Procedure，`my_amount_bills` 不在 Billing 域的订阅里，靠 `bill_id`
/// 反查会永远查不到，这一栏就永远不显示。
///
/// `editing_bill_id` 是正在编辑的账单（新建传 0）：已被它自己占用的项要留在
/// 列表里且保持勾选，否则编辑一次账单就会把自己并入的结转弄丢。
#[pure_function::pure]
pub(super) fn available_carryover(
    items: &[CarryoverItem],
    batches: &[CarryoverBatch],
    tenant_id: u64,
    editing_bill_id: u64,
) -> Vec<CarryoverOption> {
    let mut options = items
        .iter()
        .filter(|item| item.disposition == DISPOSITION_CARRY)
        .filter(|item| match item.consumed_by_bill_id {
            None => true,
            Some(owner) => owner == editing_bill_id && editing_bill_id != 0,
        })
        .filter(|item| {
            batches
                .iter()
                .any(|batch| batch.batch_id == item.batch_id && batch.status == BATCH_CONFIRMED)
        })
        .filter_map(|item| {
            // 只列本租户的欠款：把甲的结转并进乙的账单是记错账，不是筛选不便。
            if tenant_id == 0 || item.tenant_id != tenant_id {
                return None;
            }
            let period_label = batches
                .iter()
                .find(|batch| batch.batch_id == item.batch_id)
                .map(|batch| batch.period_label.clone())
                .unwrap_or_default();
            Some(CarryoverOption {
                item_id: item.item_id,
                period_label,
                project_name: item
                    .source_project_name
                    .clone()
                    .unwrap_or_else(|| format!("账单 #{}", item.bill_id)),
                shortfall_cents: carryover_amount_cents(item),
            })
        })
        .collect::<Vec<_>>();
    // 账期升序：先欠的先还，和催缴时的说法一致。
    options.sort_by(|a, b| {
        a.period_label
            .cmp(&b.period_label)
            .then(a.item_id.cmp(&b.item_id))
    });
    options
}

/// 勾选项的合计。必须与账单填写的结转额一致，否则服务端拒绝。
#[pure_function::pure]
pub(super) fn selected_carryover_cents(options: &[CarryoverOption], selected: &[u64]) -> i64 {
    options
        .iter()
        .filter(|option| selected.contains(&option.item_id))
        .fold(0i64, |sum, option| sum.saturating_add(option.shortfall_cents))
}

/// 结转项的名目，如「2026-07 欠款结转」；跨多个账期时并列。
///
/// 返回 `None` 表示没有勾选任何结转——此时账单不该带结转名目，留一个空名目
/// 在对外的账单上比不写更费解。
#[pure_function::pure]
pub(super) fn carryover_item_label(options: &[CarryoverOption], selected: &[u64]) -> Option<String> {
    let mut periods = options
        .iter()
        .filter(|option| selected.contains(&option.item_id))
        .map(|option| option.period_label.as_str())
        .collect::<Vec<_>>();
    if periods.is_empty() {
        return None;
    }
    periods.dedup();
    Some(format!("{} 欠款结转", periods.join("、")))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BillSummary {
    pub total_cents: i64,
    pub receipt_cents: i64,
    pub remaining_cents: i64,
    pub overpaid_cents: i64,
}

#[pure_function::pure]
pub(super) fn collection_status(bill: &AmountBill) -> CollectionStatus {
    collection_status_from_amounts(bill.total_fee_cents, bill.receipt_amount_cents)
}

fn collection_status_from_amounts(
    total_fee_cents: i64,
    receipt_amount_cents: i64,
) -> CollectionStatus {
    match receipt_amount_cents.cmp(&total_fee_cents) {
        std::cmp::Ordering::Greater => CollectionStatus::Overpaid,
        std::cmp::Ordering::Equal if total_fee_cents > 0 => CollectionStatus::Paid,
        std::cmp::Ordering::Less if receipt_amount_cents > 0 => CollectionStatus::Partial,
        _ => CollectionStatus::Unpaid,
    }
}

#[pure_function::pure]
pub(super) fn remaining_cents(bill: &AmountBill) -> i64 {
    bill.total_fee_cents
        .saturating_sub(bill.receipt_amount_cents)
        .max(0)
}

#[pure_function::pure]
pub(super) fn overpaid_cents(bill: &AmountBill) -> i64 {
    bill.receipt_amount_cents
        .saturating_sub(bill.total_fee_cents)
        .max(0)
}

#[pure_function::pure]
pub(super) fn summarize<'a>(bills: impl Iterator<Item = &'a AmountBill>) -> BillSummary {
    bills.fold(BillSummary::default(), |mut summary, bill| {
        summary.total_cents = summary.total_cents.saturating_add(bill.total_fee_cents);
        summary.receipt_cents = summary
            .receipt_cents
            .saturating_add(bill.receipt_amount_cents);
        summary.remaining_cents = summary
            .remaining_cents
            .saturating_add(remaining_cents(bill));
        summary.overpaid_cents = summary.overpaid_cents.saturating_add(overpaid_cents(bill));
        summary
    })
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
pub(super) fn amount_input_value(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.unsigned_abs() % 100)
}

#[pure_function::pure]
pub(super) fn optional_amount_input_value(cents: Option<i64>) -> String {
    cents.map(amount_input_value).unwrap_or_default()
}

#[pure_function::pure]
pub(super) fn parse_amount_to_cents(value: &str, field: &str) -> Result<i64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    if value.starts_with('-') {
        return Err(format!("{field}不能为负数"));
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|part| !part.is_empty() && part.chars().all(|value| value.is_ascii_digit()))
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(|| format!("{field}格式不正确"))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() || fraction.len() > 2 || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!("{field}最多保留两位小数"));
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().unwrap_or_default() * 10,
        _ => fraction.parse::<i64>().unwrap_or_default(),
    };
    whole
        .checked_mul(100)
        .and_then(|value| value.checked_add(fraction))
        .ok_or_else(|| format!("{field}数值过大"))
}

#[pure_function::pure]
pub(super) fn format_date(timestamp: Option<Timestamp>) -> String {
    let Some(timestamp) = timestamp else {
        return "--".into();
    };
    let local_micros = timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let (year, month, day) = civil_from_days(local_micros.div_euclid(MICROS_PER_DAY));
    format!("{year:04}-{month:02}-{day:02}")
}

#[pure_function::pure]
pub(super) fn format_datetime(timestamp: Timestamp) -> String {
    let local_micros = timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let total_seconds = local_micros.div_euclid(1_000_000);
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        seconds / 3_600,
        seconds % 3_600 / 60
    )
}

#[pure_function::pure]
pub(super) fn parse_optional_date(value: &str) -> Result<Option<Timestamp>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let mut parts = value.split('-');
    let year = parts.next().and_then(|part| part.parse::<i32>().ok());
    let month = parts.next().and_then(|part| part.parse::<u32>().ok());
    let day = parts.next().and_then(|part| part.parse::<u32>().ok());
    if parts.next().is_some() {
        return Err("收款日期格式不正确".into());
    }
    let days = year
        .zip(month)
        .zip(day)
        .and_then(|((year, month), day)| days_from_civil(year, month, day))
        .ok_or_else(|| "请选择正确的收款日期".to_string())?;
    Ok(Some(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    )))
}

/// “新增下月”优先识别 `2026年7月` 和 `2026-07`，无法识别时保留名称并追加提示。
#[pure_function::pure]
pub(super) fn next_month_project_name(value: &str) -> String {
    if let Some(year_at) = value.find('年') {
        let mut year_start = year_at;
        while year_start > 0 && value.as_bytes()[year_start - 1].is_ascii_digit() {
            year_start -= 1;
        }
        let year = value[year_start..year_at].parse::<i32>().ok();
        let month_end = value[year_at + '年'.len_utf8()..]
            .find('月')
            .map(|index| year_at + '年'.len_utf8() + index);
        if let (Some(year), Some(month_end)) = (year, month_end) {
            let month_start = year_at + '年'.len_utf8();
            if let Ok(month) = value[month_start..month_end].parse::<u32>() {
                if (1..=12).contains(&month) {
                    let next_year = year + i32::from(month == 12);
                    let next_month = month % 12 + 1;
                    return format!(
                        "{}{}年{}月{}",
                        &value[..year_start],
                        next_year,
                        next_month,
                        &value[month_end + '月'.len_utf8()..]
                    );
                }
            }
        }
    }
    let bytes = value.as_bytes();
    for index in 4..bytes.len().saturating_sub(2) {
        if bytes[index] == b'-'
            && bytes[index - 4..index].iter().all(u8::is_ascii_digit)
            && bytes[index + 1..index + 3].iter().all(u8::is_ascii_digit)
        {
            let year_start = index - 4;
            let month_end = index + 3;
            if let (Ok(year), Ok(month)) = (
                value[year_start..index].parse::<i32>(),
                value[index + 1..month_end].parse::<u32>(),
            ) {
                if (1..=12).contains(&month) {
                    let next_year = year + i32::from(month == 12);
                    let next_month = month % 12 + 1;
                    return format!(
                        "{}{:04}-{:02}{}",
                        &value[..year_start],
                        next_year,
                        next_month,
                        &value[month_end..]
                    );
                }
            }
        }
    }
    format!("{}（下月）", value.trim())
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

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 收款状态按应收实收关系计算() {
        assert_eq!(
            collection_status_from_amounts(100, 0),
            CollectionStatus::Unpaid
        );
        assert_eq!(
            collection_status_from_amounts(100, 50),
            CollectionStatus::Partial
        );
        assert_eq!(
            collection_status_from_amounts(100, 100),
            CollectionStatus::Paid
        );
        assert_eq!(
            collection_status_from_amounts(100, 120),
            CollectionStatus::Overpaid
        );
    }

    #[test]
    fn 新增下月可以跨年() {
        assert_eq!(
            next_month_project_name("2026年12月租金账单"),
            "2027年1月租金账单"
        );
        assert_eq!(next_month_project_name("账单 2026-07"), "账单 2026-08");
    }

    fn bill_with(total: i64, receipt: i64, has_time: bool) -> AmountBill {
        let mut bill = AmountBill {
            bill_id: 1,
            customer_id: "c".into(),
            project_name: "测试".into(),
            tenant_name: None,
            public_bank_account: None,
            private_bank_account: None,
            ele_fee_cents: 0,
            water_fee_cents: 0,
            receive_fee_cents: 0,
            factory_rent_cents: 0,
            management_fee_cents: 0,
            invoice_tax_cents: 0,
            total_fee_cents: total,
            service_fee_cents: 0,
            garbage_fee_cents: 0,
            extra_ele_fee_cents: 0,
            basic_ele_fee_cents: 0,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            receipt_amount_cents: receipt,
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
            finance_id: 1,
            tenant_id: 1,
            park_id: 1,
            created_at: Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
            carryover_fee_cents: 0,
            carryover_item: None,
        };
        if has_time {
            bill.receipt_time = Some(Timestamp::from_micros_since_unix_epoch(1));
        }
        bill
    }

    #[test]
    fn 对账状态与服务端清单一致() {
        use ReconciliationState::*;
        // 没有收款动作一律待收款——应收为 0 的历史账单不能一建出来就变待确认。
        assert_eq!(reconciliation_state(&bill_with(0, 0, false), false, false), AwaitingReceipt);
        assert_eq!(reconciliation_state(&bill_with(100, 0, false), false, false), AwaitingReceipt);
        // 实收达标（含多缴）即待确认收齐；催缴期间补齐了同样优先走确认收齐。
        assert_eq!(reconciliation_state(&bill_with(100, 100, false), false, false), AwaitingCollectedConfirm);
        assert_eq!(reconciliation_state(&bill_with(100, 120, false), false, true), AwaitingCollectedConfirm);
        // 部分收款：先待确认差额，确认后进入催交中。
        assert_eq!(reconciliation_state(&bill_with(100, 40, false), false, false), AwaitingShortfallConfirm);
        assert_eq!(reconciliation_state(&bill_with(100, 40, false), false, true), ShortfallChasing);
        // 收齐确认是终态，压过一切。
        assert_eq!(reconciliation_state(&bill_with(100, 10, true), true, true), Closed);
    }

    #[test]
    fn 确认按钮的允许状态与服务端动作清单一致() {
        use ReconciliationState::*;
        assert!(AwaitingCollectedConfirm.can_confirm_collected());
        assert!(!AwaitingShortfallConfirm.can_confirm_collected());
        assert!(!Closed.can_confirm_collected());
        assert!(AwaitingShortfallConfirm.can_confirm_shortfall());
        assert!(ShortfallChasing.can_confirm_shortfall());
        assert!(!AwaitingCollectedConfirm.can_confirm_shortfall());
        assert!(!Closed.can_confirm_shortfall());
    }

    #[test]
    fn 确认记录按账单聚合出两个旗标() {
        let mut collected = BillCollectionConfirmation {
            confirmation_id: 1,
            customer_id: "c".into(),
            bill_id: 7,
            confirm_type: CONFIRM_TYPE_COLLECTED.into(),
            receivable_cents: 100,
            received_cents: 100,
            confirmed_by: 1,
            confirmed_by_name: "经理".into(),
            remark: None,
            confirmed_at: Timestamp::from_micros_since_unix_epoch(0),
        };
        let mut shortfall = collected.clone();
        shortfall.confirmation_id = 2;
        shortfall.bill_id = 8;
        shortfall.confirm_type = CONFIRM_TYPE_SHORTFALL.into();
        collected.bill_id = 7;
        let flags = confirmation_flags(&[collected, shortfall]);
        assert_eq!(flags.get(&7), Some(&(true, false)));
        assert_eq!(flags.get(&8), Some(&(false, true)));
        assert_eq!(flags.get(&9), None);
    }

    fn item_with(disposition: &str, receivable: i64, received: i64) -> CarryoverItem {
        CarryoverItem {
            item_id: 1,
            customer_id: "c".into(),
            batch_id: 1,
            bill_id: 1,
            receivable_cents: receivable,
            received_cents: received,
            disposition: disposition.into(),
            consumed_by_bill_id: None,
            tenant_id: 0,
            source_project_name: None,
            created_at: Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
        }
    }

    #[test]
    fn 只有结转处置才产生金额() {
        assert_eq!(carryover_amount_cents(&item_with(DISPOSITION_CARRY, 220_000, 40)), 219_960);
        assert_eq!(carryover_amount_cents(&item_with(DISPOSITION_SKIP, 220_000, 40)), 0);
        assert_eq!(carryover_amount_cents(&item_with(DISPOSITION_COLLECTED, 220_000, 40)), 0);
        // 多缴不产生负数结转，否则下个月会凭空少收一笔。
        assert_eq!(carryover_amount_cents(&item_with(DISPOSITION_CARRY, 100, 120)), 0);
    }

    fn full_item(item_id: u64, batch: u64, bill: u64, consumed: Option<u64>) -> CarryoverItem {
        let mut row = item_with(DISPOSITION_CARRY, 220_000, 20_000);
        row.item_id = item_id;
        row.batch_id = batch;
        row.bill_id = bill;
        row.consumed_by_bill_id = consumed;
        row.tenant_id = if item_id == 4 { 8 } else { 7 };
        row.source_project_name = Some(format!("七月租金 #{bill}"));
        row
    }

    fn batch_with(batch_id: u64, status: &str, period: &str) -> CarryoverBatch {
        CarryoverBatch {
            batch_id,
            customer_id: "c".into(),
            park_id: 1,
            period_label: period.into(),
            status: status.into(),
            created_at: Timestamp::from_micros_since_unix_epoch(0),
            confirmed_by: None,
            confirmed_by_name: None,
            confirmed_at: None,
        }
    }

    fn bill_of(bill_id: u64, tenant_id: u64, name: &str) -> AmountBill {
        let mut row = bill_with(0, 0, false);
        row.bill_id = bill_id;
        row.tenant_id = tenant_id;
        row.project_name = name.into();
        row
    }

    #[test]
    fn 只列出已确认批次里未被占用的本租户结转() {
        let items = vec![
            full_item(1, 1, 11, None),      // 合格
            full_item(2, 2, 12, None),      // 批次未确认
            full_item(3, 1, 13, Some(99)),  // 已被别的账单占用
            full_item(4, 1, 14, None),      // 别的租户
        ];
        let batches = vec![
            batch_with(1, BATCH_CONFIRMED, "2026-07"),
            batch_with(2, BATCH_PENDING, "2026-07"),
        ];
        let bills = vec![
            bill_of(11, 7, "七月租金"),
            bill_of(12, 7, "七月租金"),
            bill_of(13, 7, "七月租金"),
            bill_of(14, 8, "别家租金"),
        ];
        let options = available_carryover(&items, &batches, 7, 0);
        assert_eq!(options.iter().map(|o| o.item_id).collect::<Vec<_>>(), vec![1]);
        assert_eq!(options[0].shortfall_cents, 200_000);
    }

    #[test]
    fn 编辑账单时自己已占用的结转仍在列表里() {
        let items = vec![full_item(3, 1, 13, Some(55))];
        let batches = vec![batch_with(1, BATCH_CONFIRMED, "2026-07")];
        let bills = vec![bill_of(13, 7, "七月租金")];
        // 换成别的账单在编辑，就不该看到它——那是别人占着的钱。
        assert!(available_carryover(&items, &batches, 7, 56).is_empty());
        assert_eq!(available_carryover(&items, &batches, 7, 55).len(), 1);
    }

    #[test]
    fn 勾选合计与名目按账期升序() {
        let items = vec![full_item(1, 1, 11, None), full_item(2, 2, 12, None)];
        let batches = vec![
            batch_with(1, BATCH_CONFIRMED, "2026-08"),
            batch_with(2, BATCH_CONFIRMED, "2026-06"),
        ];
        let bills = vec![bill_of(11, 7, "八月"), bill_of(12, 7, "六月")];
        let options = available_carryover(&items, &batches, 7, 0);
        assert_eq!(
            options.iter().map(|o| o.period_label.as_str()).collect::<Vec<_>>(),
            vec!["2026-06", "2026-08"],
            "先欠的要排在前面"
        );
        assert_eq!(selected_carryover_cents(&options, &[1, 2]), 400_000);
        assert_eq!(selected_carryover_cents(&options, &[2]), 200_000);
        assert_eq!(
            carryover_item_label(&options, &[1, 2]).as_deref(),
            Some("2026-06、2026-08 欠款结转")
        );
    }

    #[test]
    fn 没勾选时不产生结转名目() {
        let options = Vec::new();
        assert_eq!(selected_carryover_cents(&options, &[]), 0);
        assert!(carryover_item_label(&options, &[]).is_none());
    }

    #[test]
    fn 批次合计只数结转那一部分() {
        let items = vec![
            item_with(DISPOSITION_CARRY, 1_000, 300),
            item_with(DISPOSITION_CARRY, 500, 0),
            item_with(DISPOSITION_SKIP, 800, 0),
            item_with(DISPOSITION_COLLECTED, 900, 900),
        ];
        assert_eq!(carryover_totals(&items), (2, 1_200));
    }

    #[test]
    fn 未知处置按默认显示而不是空白() {
        assert_eq!(disposition_label(DISPOSITION_CARRY).0, "结转到下期");
        assert_eq!(disposition_label(DISPOSITION_SKIP).0, "本次不结转");
        assert_eq!(disposition_label(DISPOSITION_COLLECTED).0, "剔除并确认收齐");
        assert_eq!(disposition_label("unknown").0, "结转到下期");
    }

    #[test]
    fn 账单金额严格换算为分() {
        assert_eq!(parse_amount_to_cents("1234.5", "金额"), Ok(123_450));
        assert_eq!(format_money(1_234_567), "¥12,345.67");
        assert!(parse_amount_to_cents("1.234", "金额").is_err());
    }
}
