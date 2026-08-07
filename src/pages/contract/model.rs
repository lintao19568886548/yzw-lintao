//! 合同日期、金额、状态与递增规则的纯计算模型。

use serde::{Deserialize, Serialize};
use spacetimedb_sdk::Timestamp;

use std::collections::BTreeMap;

use crate::spacetime_bindings::{
    dormitory_floor_type::DormitoryFloor, dormitory_type::Dormitory,
    factory_floor_type::FactoryFloor, factory_type::Factory,
    rental_tenant_dormitory_floor_type::RentalTenantDormitoryFloor,
    rental_tenant_floor_type::RentalTenantFloor,
    rental_tenant_fee_input_type::RentalTenantFeeInput, rental_tenant_fee_type::RentalTenantFee,
    rental_tenant_meter_input_type::RentalTenantMeterInput,
    rental_tenant_meter_type::RentalTenantMeter, rental_tenant_type::RentalTenant,
    utility_meter_type::UtilityMeter,
};

const MICROS_PER_DAY: i64 = 86_400_000_000;
const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContractStatus {
    Active,
    Expiring,
    Expired,
    MissingDate,
}

impl ContractStatus {
    #[pure_function::pure]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Active => "生效中",
            Self::Expiring => "即将到期",
            Self::Expired => "已过期",
            Self::MissingDate => "待完善",
        }
    }

    /// 状态到徽章：生效中实心，即将到期和已过期用危险色，待完善只描边。
    #[pure_function::pure]
    pub(super) fn badge_variant(self) -> crate::components::badge::BadgeVariant {
        use crate::components::badge::BadgeVariant;
        match self {
            Self::Active => BadgeVariant::Secondary,
            Self::Expiring | Self::Expired => BadgeVariant::Destructive,
            Self::MissingDate => BadgeVariant::Outline,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct IncreaseRule {
    pub date: u32,
    pub rate: f64,
}

pub(super) fn today_timestamp() -> Timestamp {
    parse_date(&crate::pages::smart_meter::date::today())
        .unwrap_or_else(|_| Timestamp::from_micros_since_unix_epoch(0))
}

#[pure_function::pure]
pub(super) fn contract_status(row: &RentalTenant, today: Timestamp) -> ContractStatus {
    let Some(end) = row.contract_end else {
        return ContractStatus::MissingDate;
    };
    let days = days_between(today, end);
    if days < 0 {
        ContractStatus::Expired
    } else if days <= 90 {
        ContractStatus::Expiring
    } else {
        ContractStatus::Active
    }
}

#[pure_function::pure]
pub(super) fn contract_reminder(row: &RentalTenant, today: Timestamp) -> String {
    let Some(end) = row.contract_end else {
        return "尚未录入合同结束日期".into();
    };
    let days = days_between(today, end);
    if days < 0 {
        format!("已过期 {} 天", days.unsigned_abs())
    } else if days == 0 {
        "今天到期".into()
    } else {
        format!("距离到期还有 {days} 天")
    }
}

/// 与原系统批量提醒条件一致：收入合同未到期，90 天内到期或 30 天内递增。
#[pure_function::pure]
pub(super) fn contract_sms_eligible(row: &RentalTenant, today: Timestamp) -> bool {
    if !row.transaction_type {
        return false;
    }
    let Some(end) = row.contract_end else {
        return false;
    };
    let end_days = days_between(today, end);
    if end_days <= 0 {
        return false;
    }
    if end_days <= 90 {
        return true;
    }
    if let Some(increase_date) = row.increase_date {
        let days = days_between(today, increase_date);
        if days > 0 && days <= 30 {
            return true;
        }
    }
    let Some(start) = row.contract_start else {
        return false;
    };
    parse_increase_rules(row.increase_data.as_deref())
        .into_iter()
        .filter_map(|rule| {
            start
                .to_micros_since_unix_epoch()
                .checked_add(i64::from(rule.date) * 365 * MICROS_PER_DAY)
                .map(Timestamp::from_micros_since_unix_epoch)
        })
        .any(|date| {
            let days = days_between(today, date);
            days > 0 && days <= 30
        })
}

#[pure_function::pure]
pub(super) fn format_date(timestamp: Option<Timestamp>) -> String {
    let Some(timestamp) = timestamp else {
        return "--".into();
    };
    let days =
        (timestamp.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS).div_euclid(MICROS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

#[pure_function::pure]
pub(super) fn parse_optional_date(value: &str, label: &str) -> Result<Option<Timestamp>, String> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_date(value)
            .map(Some)
            .map_err(|_| format!("请选择正确的{label}"))
    }
}

fn parse_date(value: &str) -> Result<Timestamp, String> {
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
        .ok_or_else(|| "日期格式不正确".to_string())?;
    Ok(Timestamp::from_micros_since_unix_epoch(
        days * MICROS_PER_DAY - CHINA_OFFSET_MICROS,
    ))
}

#[pure_function::pure]
pub(super) fn format_money(cents: Option<i64>) -> String {
    cents
        .map(|value| format!("¥{}.{:02}", value / 100, value.unsigned_abs() % 100))
        .unwrap_or_else(|| "--".into())
}

#[pure_function::pure]
pub(super) fn money_input(cents: Option<i64>) -> String {
    cents
        .map(|value| format!("{}.{:02}", value / 100, value.unsigned_abs() % 100))
        .unwrap_or_default()
}

#[pure_function::pure]
pub(super) fn decimal_input(value: Option<i64>, scale: i64) -> String {
    value
        .map(|value| {
            let whole = value / scale;
            let fraction = (value % scale).unsigned_abs();
            if fraction == 0 {
                whole.to_string()
            } else {
                format!("{whole}.{:02}", fraction)
                    .trim_end_matches('0')
                    .to_string()
            }
        })
        .unwrap_or_default()
}

#[pure_function::pure]
pub(super) fn parse_decimal(value: &str, scale: i64, label: &str) -> Result<Option<i64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.starts_with('-') {
        return Err(format!("{label}不能为负数"));
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(|| format!("{label}格式不正确"))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || fraction.len() > 2
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{label}最多保留两位小数"));
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().unwrap_or_default() * 10,
        _ => fraction.parse::<i64>().unwrap_or_default(),
    };
    whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction * scale / 100))
        .map(Some)
        .ok_or_else(|| format!("{label}数值过大"))
}

/// 水电单价的存储倍数，与服务端 `unit_price_scaled` 一致。
///
/// 原系统这一列是 MySQL `decimal(10,8)`，不是两位小数的金额。
pub(super) const UNIT_PRICE_SCALE: i64 = 100_000_000;

/// 水电单价的输入解析，最多八位小数。
///
/// 不复用 [`parse_decimal`]：那个函数写死了"最多保留两位小数"，是给租金
/// 这类金额用的。电价 0.6483 元/度这种四位小数在生产账单里随处可见，用
/// 它解析会被直接拒掉。
#[pure_function::pure]
pub(super) fn parse_unit_price(value: &str, label: &str) -> Result<Option<i64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.starts_with('-') {
        return Err(format!("{label}不能为负数"));
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|part| part.parse::<i64>().ok())
        .ok_or_else(|| format!("{label}格式不正确"))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || fraction.len() > 8
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{label}最多保留八位小数"));
    }
    // 右侧补零到八位："1.05" 的小数部分是 05000000，不是 5。
    let padded = format!("{fraction:0<8}");
    let fraction = padded.parse::<i64>().unwrap_or_default();
    whole
        .checked_mul(UNIT_PRICE_SCALE)
        .and_then(|value| value.checked_add(fraction))
        .map(Some)
        .ok_or_else(|| format!("{label}数值过大"))
}

/// 把存储值还原成输入框文本，去掉尾部多余的零。
#[pure_function::pure]
pub(super) fn format_unit_price(value: Option<i64>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let whole = value / UNIT_PRICE_SCALE;
    let fraction = (value % UNIT_PRICE_SCALE).unsigned_abs();
    if fraction == 0 {
        return whole.to_string();
    }
    let text = format!("{fraction:08}");
    format!("{whole}.{}", text.trim_end_matches('0'))
}

/// 合同对一块水电表的报价草稿，即表单里那几个输入框的文本状态。
///
/// 单独建模而不是直接拼 `RentalTenantMeterInput`，是为了让"单一价还是
/// 分时四段"这个二选一在提交之前就能判定并给出具体的错误——服务端也会
/// 再拦一次，但等一次往返回来才告诉用户"分时电价必须四段全填"，用户已经
/// 关掉抽屉了。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct MeterPriceDraft {
    /// 勾选后按尖峰平谷四段报价，否则只用 `unit_price`。
    pub(super) tiered: bool,
    pub(super) unit_price: String,
    pub(super) tip: String,
    pub(super) peak: String,
    pub(super) flat: String,
    pub(super) valley: String,
}

impl MeterPriceDraft {
    /// 从已保存的关联行还原表单状态。
    #[pure_function::pure]
    pub(super) fn from_link(link: &RentalTenantMeter) -> Self {
        Self {
            tiered: link.price_flat_scaled.is_some(),
            unit_price: format_unit_price(link.unit_price_scaled),
            tip: format_unit_price(link.price_tip_scaled),
            peak: format_unit_price(link.price_peak_scaled),
            flat: format_unit_price(link.price_flat_scaled),
            valley: format_unit_price(link.price_valley_scaled),
        }
    }

    /// 校验并转成提交给服务端的报价。
    #[pure_function::pure]
    pub(super) fn to_input(
        &self,
        meter_id: u64,
        meter_code: &str,
    ) -> Result<RentalTenantMeterInput, String> {
        let mut input = RentalTenantMeterInput {
            meter_id,
            unit_price_scaled: None,
            price_tip_scaled: None,
            price_peak_scaled: None,
            price_flat_scaled: None,
            price_valley_scaled: None,
            remark: None,
        };
        if self.tiered {
            let tiers = [
                (&self.tip, "尖"),
                (&self.peak, "峰"),
                (&self.flat, "平"),
                (&self.valley, "谷"),
            ];
            let mut parsed = [0_i64; 4];
            for (index, (text, name)) in tiers.into_iter().enumerate() {
                parsed[index] = parse_unit_price(text, &format!("「{meter_code}」的{name}段电价"))?
                    .ok_or_else(|| {
                        format!("「{meter_code}」按分时报价时，尖峰平谷四段都要填")
                    })?;
            }
            input.price_tip_scaled = Some(parsed[0]);
            input.price_peak_scaled = Some(parsed[1]);
            input.price_flat_scaled = Some(parsed[2]);
            input.price_valley_scaled = Some(parsed[3]);
        } else {
            input.unit_price_scaled = Some(
                parse_unit_price(&self.unit_price, &format!("「{meter_code}」的单价"))?
                    .ok_or_else(|| format!("请填写「{meter_code}」的结算单价"))?,
            );
        }
        Ok(input)
    }
}

/// 合同能约定的周期性费用，顺序即表单里的显示顺序。
///
/// 与服务端 `tables::rental::tenant_fee` 的 `FEE_KINDS` 一一对应。
pub(super) const FEE_KINDS: [(&str, &str); 3] = [
    ("loss", "电损费"),
    ("service", "服务费"),
    ("garbage", "垃圾费"),
];

/// 比例项的计费基数，顺序即下拉框里的顺序。
pub(super) const RATE_BASES: [(&str, &str); 5] = [
    ("factory_fee", "厂房电费合计"),
    ("factory_fee_with_basic", "厂房电费 + 基本电费"),
    ("factory_dorm_fee", "厂房宿舍电费合计"),
    ("total_ele_fee", "全部电费合计"),
    ("factory_usage", "厂房度数合计"),
];

#[pure_function::pure]
pub(super) fn fee_kind_label(fee_kind: &str) -> &'static str {
    FEE_KINDS
        .iter()
        .find(|(kind, _)| *kind == fee_kind)
        .map(|(_, label)| *label)
        .unwrap_or("未知费用")
}

/// 合同约定一项费用的草稿，即表单里那一行的文本状态。
///
/// 和 [`MeterPriceDraft`] 同样的理由单独建模：「按比例就必须选基数」这类
/// 约束要在提交前判定，否则用户关掉抽屉之后才收到服务端的错误。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FeeDraft {
    /// 未勾选表示这份合同不收这一项。
    pub(super) enabled: bool,
    /// 勾选后按比例计费，否则按固定金额。
    pub(super) by_rate: bool,
    /// 固定金额，元。
    pub(super) amount: String,
    /// 比例，百分数。
    pub(super) rate: String,
    pub(super) rate_base: String,
}

impl FeeDraft {
    /// 从已保存的约定还原表单状态。
    #[pure_function::pure]
    pub(super) fn from_row(row: &RentalTenantFee) -> Self {
        let by_rate = row.charge_mode == "rate";
        Self {
            enabled: true,
            by_rate,
            amount: money_input(row.amount_cents),
            // 基点转百分数：`525` 是 `5.25%`。
            rate: decimal_input(row.rate_basis_points, 100),
            rate_base: row.rate_base.clone().unwrap_or_default(),
        }
    }

    /// 校验并转成提交给服务端的约定；未启用时返回 `None`。
    #[pure_function::pure]
    pub(super) fn to_input(&self, fee_kind: &str) -> Result<Option<RentalTenantFeeInput>, String> {
        if !self.enabled {
            return Ok(None);
        }
        let label = fee_kind_label(fee_kind);
        let mut input = RentalTenantFeeInput {
            fee_kind: fee_kind.to_string(),
            charge_mode: if self.by_rate { "rate" } else { "fixed" }.to_string(),
            amount_cents: None,
            rate_basis_points: None,
            rate_base: None,
            remark: None,
        };
        if self.by_rate {
            input.rate_basis_points = Some(
                parse_decimal(&self.rate, 100, &format!("{label}比例"))?
                    .ok_or_else(|| format!("请填写{label}的比例"))?,
            );
            if self.rate_base.is_empty() {
                return Err(format!("请选择{label}的计费基数"));
            }
            input.rate_base = Some(self.rate_base.clone());
        } else {
            input.amount_cents = Some(
                parse_decimal(&self.amount, 100, &format!("{label}金额"))?
                    .ok_or_else(|| format!("请填写{label}的金额"))?,
            );
        }
        Ok(Some(input))
    }
}

/// 每份合同租在哪里，由楼层关联算出。
///
/// 这取代了原来 `RentalTenant.address` 那个自由文本字段——建立楼层关联表
/// 之前，「这份合同租的是哪一层」只写在那里（"A栋一楼"、"佛山A栋102"），
/// 既没法程序化关联，选完楼层之后还要再手抄一遍、两边各自漂移。
///
/// 一次算出全部合同的说明而不是逐份查：合同列表一屏几十行，逐行扫一遍
/// 关联表是平方级的。
#[pure_function::pure]
pub(crate) fn contract_locations(
    factories: &[Factory],
    factory_floors: &[FactoryFloor],
    tenant_floors: &[RentalTenantFloor],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
    tenant_dormitory_floors: &[RentalTenantDormitoryFloor],
) -> BTreeMap<u64, String> {
    let mut parts = BTreeMap::<u64, Vec<String>>::new();

    for link in tenant_floors {
        let Some(floor) = factory_floors
            .iter()
            .find(|floor| floor.floor_id == link.floor_id)
        else {
            continue;
        };
        let building = factories
            .iter()
            .find(|factory| factory.factory_id == floor.factory_id)
            .map(|factory| factory.factory_name.as_str())
            .unwrap_or("未知厂房");
        parts
            .entry(link.rental_tenant_id)
            .or_default()
            .push(format!("{building} · {}", floor.floor_name));
    }

    for link in tenant_dormitory_floors {
        let Some(floor) = dormitory_floors
            .iter()
            .find(|floor| floor.dormitory_floor_id == link.dormitory_floor_id)
        else {
            continue;
        };
        let building = dormitories
            .iter()
            .find(|dormitory| dormitory.dormitory_id == floor.dormitory_id)
            .map(|dormitory| dormitory.dormitory_name.as_str())
            .unwrap_or("未知宿舍");
        parts.entry(link.rental_tenant_id).or_default().push(format!(
            "{building} · {} 层 {} 间",
            floor.floor_no, link.room_count
        ));
    }

    parts
        .into_iter()
        .map(|(id, mut labels)| {
            labels.sort();
            labels.dedup();
            (id, labels.join("、"))
        })
        .collect()
}

/// 水电表装在哪里，给合同表单的选择列表用。
///
/// 两个位置都为空是合法的：园区公共区域的表不属于任何一层（生产账单里
/// 「公共用电」那 299 条就是这一类），这里明确标出来，避免它们看起来像
/// 是漏填了安装位置的脏数据。
#[pure_function::pure]
pub(crate) fn meter_location_label(
    meter: &UtilityMeter,
    factories: &[Factory],
    factory_floors: &[FactoryFloor],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
) -> String {
    if meter.factory_floor_id != 0 {
        return factory_floors
            .iter()
            .find(|floor| floor.floor_id == meter.factory_floor_id)
            .map(|floor| {
                let building = factories
                    .iter()
                    .find(|factory| factory.factory_id == floor.factory_id)
                    .map(|factory| factory.factory_name.clone())
                    .unwrap_or_else(|| "未知厂房".into());
                format!("{building} · {}", floor.floor_name)
            })
            .unwrap_or_else(|| "楼层已删除".into());
    }
    if meter.dormitory_floor_id != 0 {
        return dormitory_floors
            .iter()
            .find(|floor| floor.dormitory_floor_id == meter.dormitory_floor_id)
            .map(|floor| {
                let building = dormitories
                    .iter()
                    .find(|dormitory| dormitory.dormitory_id == floor.dormitory_id)
                    .map(|dormitory| dormitory.dormitory_name.clone())
                    .unwrap_or_else(|| "未知宿舍".into());
                format!("{building} · {} 层", floor.floor_no)
            })
            .unwrap_or_else(|| "楼层已删除".into());
    }
    "园区公共区域".into()
}

#[pure_function::pure]
pub(super) fn parse_increase_rules(value: Option<&str>) -> Vec<IncreaseRule> {
    value
        .and_then(|value| serde_json::from_str::<Vec<IncreaseRule>>(value).ok())
        .unwrap_or_default()
}

#[pure_function::pure]
pub(super) fn serialize_increase_rules(rules: &[IncreaseRule]) -> Result<Option<String>, String> {
    let rules = rules
        .iter()
        .filter(|rule| rule.date > 0 && rule.rate >= 0.0)
        .cloned()
        .collect::<Vec<_>>();
    if rules.is_empty() {
        return Ok(None);
    }
    serde_json::to_string(&rules)
        .map(Some)
        .map_err(|error| format!("保存递增规则失败：{error}"))
}

#[pure_function::pure]
pub(super) fn increase_summary(row: &RentalTenant) -> String {
    let rules = parse_increase_rules(row.increase_data.as_deref());
    if rules.is_empty() {
        return "无递增规则".into();
    }
    rules
        .iter()
        .take(3)
        .map(|rule| format!("第{}年 +{}%", rule.date, trim_float(rule.rate)))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn trim_float(value: f64) -> String {
    let value = format!("{value:.2}");
    value
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn days_between(start: Timestamp, end: Timestamp) -> i64 {
    let start =
        (start.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS).div_euclid(MICROS_PER_DAY);
    let end = (end.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS).div_euclid(MICROS_PER_DAY);
    end - start
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
    let adjusted_year = year - i32::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097 + day_of_era - 719_468) as i64)
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
    fn 合同日期和金额可以稳定换算() {
        let date = parse_date("2026-07-13").unwrap();
        assert_eq!(format_date(Some(date)), "2026-07-13");
        assert_eq!(parse_decimal("1234.5", 100, "租金"), Ok(Some(123_450)));
    }

    #[test]
    fn 水电单价保留八位小数() {
        // 电价 0.6483 元/度这种精度用租金那套两位小数的解析会被直接拒掉。
        assert_eq!(parse_unit_price("0.6483", "电价"), Ok(Some(64_830_000)));
        assert_eq!(parse_unit_price("1.05", "电价"), Ok(Some(105_000_000)));
        assert_eq!(parse_unit_price("3", "电价"), Ok(Some(300_000_000)));
        assert!(parse_unit_price("0.123456789", "电价").is_err());
        assert!(parse_unit_price("-1", "电价").is_err());
    }

    #[test]
    fn 水电单价往返不丢精度() {
        for text in ["0.6483", "1.05", "3", "12.00000001"] {
            let scaled = parse_unit_price(text, "电价").unwrap();
            assert_eq!(format_unit_price(scaled), text, "{text}");
        }
        assert_eq!(format_unit_price(None), "");
    }

    #[test]
    fn 分时报价必须四段填全() {
        let mut draft = MeterPriceDraft {
            tiered: true,
            tip: "1.5".into(),
            peak: "1.2".into(),
            flat: "0.9".into(),
            valley: "0.5".into(),
            ..Default::default()
        };
        assert!(draft.to_input(1, "A1").is_ok());
        draft.valley.clear();
        let error = draft.to_input(1, "A1").unwrap_err();
        assert!(error.contains("四段都要填"), "{error}");
    }

    #[test]
    fn 单一报价与分时报价互不串味() {
        // 切到分时之后，单价框里残留的文本不能跟着一起提交。
        let draft = MeterPriceDraft {
            tiered: true,
            unit_price: "9.99".into(),
            tip: "1.5".into(),
            peak: "1.2".into(),
            flat: "0.9".into(),
            valley: "0.5".into(),
        };
        let input = draft.to_input(1, "A1").unwrap();
        assert_eq!(input.unit_price_scaled, None);
        assert_eq!(input.price_flat_scaled, Some(90_000_000));

        let single = MeterPriceDraft { unit_price: "1.05".into(), ..Default::default() };
        let input = single.to_input(1, "A1").unwrap();
        assert_eq!(input.unit_price_scaled, Some(105_000_000));
        assert_eq!(input.price_tip_scaled, None);
    }

    #[test]
    fn 未填单价被拦在提交之前() {
        assert!(MeterPriceDraft::default().to_input(1, "A1").is_err());
    }

    fn ts() -> Timestamp {
        Timestamp::from_micros_since_unix_epoch(0)
    }

    fn a_factory(id: u64, name: &str) -> Factory {
        Factory {
            factory_id: id,
            customer_id: String::new(),
            factory_name: name.into(),
            park_id: 1,
            build_date: None,
            description: None,
            is_own: true,
            is_deleted: false,
            created_at: ts(),
            updated_at: None,
        }
    }

    fn a_floor(id: u64, factory_id: u64, name: &str) -> FactoryFloor {
        FactoryFloor {
            floor_id: id,
            customer_id: String::new(),
            factory_id,
            floor_name: name.into(),
            floor_height_centi_metres: None,
            load_bearing_centi_units: None,
            rent_price_cents: 0,
            total_area_centi_square_metres: 0,
            description: None,
            is_deleted: false,
            created_at: ts(),
            updated_at: None,
        }
    }

    fn a_link(tenant: u64, floor_id: u64) -> RentalTenantFloor {
        RentalTenantFloor {
            id: floor_id * 100 + tenant,
            customer_id: String::new(),
            rental_tenant_id: tenant,
            floor_id,
            area_centi_square_metres: 0,
            created_at: ts(),
            updated_at: None,
        }
    }

    #[test]
    fn 跨多层的合同位置合并成一行() {
        let locations = contract_locations(
            &[a_factory(1, "A型厂房")],
            &[a_floor(10, 1, "一楼"), a_floor(11, 1, "二楼")],
            &[a_link(7, 10), a_link(7, 11)],
            &[],
            &[],
            &[],
        );
        assert_eq!(locations.get(&7).map(String::as_str), Some("A型厂房 · 一楼、A型厂房 · 二楼"));
    }

    #[test]
    fn 没选楼层的合同不出现在结果里() {
        // 调用方据此显示「未选定楼层」，而不是一个空字符串。
        let locations = contract_locations(&[], &[], &[], &[], &[], &[]);
        assert!(locations.get(&7).is_none());
    }

    #[test]
    fn 指向已删楼层的关联被跳过() {
        // 楼层台账里找不到这一层时不能编出一个假名字。
        let locations = contract_locations(
            &[a_factory(1, "A型厂房")],
            &[],
            &[a_link(7, 999)],
            &[],
            &[],
            &[],
        );
        assert!(locations.get(&7).is_none());
    }

    #[test]
    fn 递增规则兼容原系统_json() {
        let rules = parse_increase_rules(Some(r#"[{"date":2,"rate":5}]"#));
        assert_eq!(rules, vec![IncreaseRule { date: 2, rate: 5.0 }]);
        assert!(serialize_increase_rules(&rules).unwrap().is_some());
    }
}
