//! 收款通知单中的水电明细工作表与银行账户结构。

use std::collections::BTreeMap;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        input::Input,
        DateField,
    },
    pages::{previous_month_same_day, today},
    services::{load_saved_token, load_smart_meter_readings, MeterKind, SmartMeterReading},
    spacetime_bindings::{
        ele_bill_type::EleBill, rental_tenant_meter_type::RentalTenantMeter,
        utility_bill_input_type::UtilityBillInput, utility_meter_type::UtilityMeter,
        water_bill_type::WaterBill,
    },
};

const CENTI_SCALE: i64 = 100;
const UNIT_PRICE_SCALE: i64 = 100_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UtilityKind {
    Electricity,
    Water,
}

impl UtilityKind {
    fn title(self) -> &'static str {
        match self {
            Self::Electricity => "电费（度）",
            Self::Water => "水费（方）",
        }
    }

    fn previous_label(self) -> &'static str {
        match self {
            Self::Electricity => "上月电表数",
            Self::Water => "上月水表数",
        }
    }

    fn usage_label(self) -> &'static str {
        match self {
            Self::Electricity => "本月际度数",
            Self::Water => "本月用水量",
        }
    }

    fn total_usage_label(self) -> &'static str {
        match self {
            Self::Electricity => "本月实际度数",
            Self::Water => "总用量",
        }
    }

    fn price_label(self) -> &'static str {
        match self {
            Self::Electricity => "单价(元/度)",
            Self::Water => "单价(元/m³)",
        }
    }

    fn amount_label(self) -> &'static str {
        match self {
            Self::Electricity => "电费金额(元)",
            Self::Water => "水费金额(元)",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct UtilityDraft {
    /// 关联的台账表主键，0 表示手工填写、不指向任何表。
    pub meter_id: u64,
    /// 分时时段标识（tip/peak/flat/valley），非分时行为 None。
    pub tou_tier: Option<String>,
    pub meter_name: String,
    pub previous_reading: String,
    pub current_reading: String,
    pub monthly_usage: String,
    pub multiplier: String,
    pub total_usage: String,
    pub unit_price: String,
    pub amount: String,
    pub remark: String,
}

impl Default for UtilityDraft {
    fn default() -> Self {
        Self {
            meter_id: 0,
            tou_tier: None,
            meter_name: String::new(),
            previous_reading: String::new(),
            current_reading: String::new(),
            monthly_usage: "0".into(),
            multiplier: "1".into(),
            total_usage: "0".into(),
            unit_price: String::new(),
            amount: "0.00".into(),
            remark: String::new(),
        }
    }
}

impl UtilityDraft {
    pub(super) fn from_ele(row: &EleBill) -> Self {
        Self::from_values(
            row.meter_id,
            row.tou_tier.clone(),
            row.meter_name.clone(),
            row.previous_reading_centi,
            row.current_reading_centi,
            row.monthly_usage_centi,
            row.multiplier_centi,
            row.total_usage_centi,
            row.unit_price_scaled,
            row.amount_cents,
            row.remark.clone(),
        )
    }

    pub(super) fn from_water(row: &WaterBill) -> Self {
        Self::from_values(
            row.meter_id,
            // 水表没有分时。
            None,
            row.meter_name.clone(),
            row.previous_reading_centi,
            row.current_reading_centi,
            row.monthly_usage_centi,
            row.multiplier_centi,
            row.total_usage_centi,
            row.unit_price_scaled,
            row.amount_cents,
            row.remark.clone(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_values(
        meter_id: u64,
        tou_tier: Option<String>,
        meter_name: String,
        previous_reading_centi: i64,
        current_reading_centi: i64,
        monthly_usage_centi: i64,
        multiplier_centi: i64,
        total_usage_centi: i64,
        unit_price_scaled: i64,
        amount_cents: i64,
        remark: Option<String>,
    ) -> Self {
        Self {
            meter_id,
            tou_tier,
            meter_name,
            previous_reading: format_scaled(previous_reading_centi, CENTI_SCALE),
            current_reading: format_scaled(current_reading_centi, CENTI_SCALE),
            monthly_usage: format_scaled(monthly_usage_centi, CENTI_SCALE),
            multiplier: format_scaled(multiplier_centi, CENTI_SCALE),
            total_usage: format_scaled(total_usage_centi, CENTI_SCALE),
            unit_price: format_scaled(unit_price_scaled, UNIT_PRICE_SCALE),
            amount: format_money_cents(amount_cents),
            remark: remark.unwrap_or_default(),
        }
    }

    fn recalculate(&mut self) {
        let previous = parse_scaled_or_zero(&self.previous_reading, CENTI_SCALE);
        let current = parse_scaled_or_zero(&self.current_reading, CENTI_SCALE);
        let multiplier = parse_scaled_or_zero(&self.multiplier, CENTI_SCALE);
        let unit_price = parse_scaled_or_zero(&self.unit_price, UNIT_PRICE_SCALE);
        let monthly = current.saturating_sub(previous).max(0);
        let total = ((monthly as i128 * multiplier as i128) / CENTI_SCALE as i128)
            .clamp(0, i64::MAX as i128) as i64;
        let amount = ((total as i128 * unit_price as i128 + UNIT_PRICE_SCALE as i128 / 2)
            / UNIT_PRICE_SCALE as i128)
            .clamp(0, i64::MAX as i128) as i64;
        self.monthly_usage = format_scaled(monthly, CENTI_SCALE);
        self.total_usage = format_scaled(total, CENTI_SCALE);
        self.amount = format_money_cents(amount);
    }
}

/// 分时时段标识，与服务端 `TOU_TIERS` 一致。库里存标识，界面显示中文。
pub(super) const TOU_TIP: &str = "tip";
pub(super) const TOU_PEAK: &str = "peak";
pub(super) const TOU_FLAT: &str = "flat";
pub(super) const TOU_VALLEY: &str = "valley";

/// 分时时段的中文叫法。
pub(super) fn tou_tier_label(tier: &str) -> &'static str {
    match tier {
        TOU_TIP => "尖",
        TOU_PEAK => "峰",
        TOU_FLAT => "平",
        TOU_VALLEY => "谷",
        _ => "未知时段",
    }
}

/// 按合同约定的用表和单价生成明细行。
///
/// 这是把水电定价搬回合同之后的收益兑现点：表号、倍率、单价全部从合同带出，
/// 用户只需要填抄见的上下期读数。在此之前这三项每个月都要重敲一遍，同一份
/// 合同连开十二个月就敲十二遍。
///
/// 分时表拆成尖／峰／平／谷四行。每行都带上 `meter_id` 和 `tou_tier`——认表
/// 靠这两个结构化字段，不再靠 `meter_name` 这个自由文本（生产库里 91 条正是
/// 把时段塞进表名的，262 条表名是空的）。表名里的中文后缀只是给人看的。
pub(super) fn drafts_from_contract(
    kind: UtilityKind,
    meters: &[UtilityMeter],
    links: &[RentalTenantMeter],
) -> Vec<UtilityDraft> {
    let want_electric = matches!(kind, UtilityKind::Electricity);
    let mut rows = Vec::new();
    for link in links {
        let Some(meter) = meters
            .iter()
            .find(|meter| meter.meter_id == link.meter_id && !meter.is_deleted)
        else {
            continue;
        };
        if meter.is_electric != want_electric {
            continue;
        }
        let multiplier = format_scaled(meter.multiplier_centi, CENTI_SCALE);
        let tiers = [
            (TOU_TIP, link.price_tip_scaled),
            (TOU_PEAK, link.price_peak_scaled),
            (TOU_FLAT, link.price_flat_scaled),
            (TOU_VALLEY, link.price_valley_scaled),
        ];
        if tiers.iter().all(|(_, price)| price.is_some()) {
            for (tier, price) in tiers {
                rows.push(UtilityDraft {
                    meter_id: meter.meter_id,
                    tou_tier: Some(tier.to_string()),
                    meter_name: format!("{} · {}", meter.meter_code, tou_tier_label(tier)),
                    multiplier: multiplier.clone(),
                    unit_price: format_scaled(price.unwrap_or_default(), UNIT_PRICE_SCALE),
                    ..Default::default()
                });
            }
        } else if let Some(price) = link.unit_price_scaled {
            rows.push(UtilityDraft {
                meter_id: meter.meter_id,
                meter_name: meter.meter_code.clone(),
                multiplier: multiplier.clone(),
                unit_price: format_scaled(price, UNIT_PRICE_SCALE),
                ..Default::default()
            });
        }
    }
    rows
}

/// 分时行该读供应商的哪一个累计寄存器。
///
/// 合众按尖／峰／平／谷四段各自回传累计值，分时表的四行必须各读各的那一段；
/// 非分时行读总量。读错段的后果是四行填成同一个数，用量全变成 0。
fn reading_value_for(reading: &SmartMeterReading, tou_tier: Option<&str>) -> String {
    match tou_tier {
        Some(TOU_TIP) => reading.data_value_tip.clone(),
        Some(TOU_PEAK) => reading.data_value_peak.clone(),
        Some(TOU_FLAT) => reading.data_value_flat.clone(),
        Some(TOU_VALLEY) => reading.data_value_valley.clone(),
        _ => reading.data_value.clone(),
    }
}

/// 把两个日期的冻结读数填进明细行，返回实际填上的行数。
///
/// 只动绑定了智能水电表设备、且该设备两个日期都有读数的行——手抄表和缺数据的
/// 行保持原样，不能因为「一键带入」就把用户已经填好的数字抹成空。
///
/// 读数是累计值：上期日期的读数进「上月表数」，本期日期的读数进「本月表数」，
/// 差值由 `recalculate` 按原表格公式算。
pub(super) fn fill_readings(
    rows: &mut [UtilityDraft],
    meters: &[UtilityMeter],
    previous: &[SmartMeterReading],
    current: &[SmartMeterReading],
) -> usize {
    let device_of = meters
        .iter()
        .filter(|meter| !meter.is_deleted)
        .filter_map(|meter| {
            meter
                .external_device_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| (meter.meter_id, id.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let index = |batch: &[SmartMeterReading]| {
        batch
            .iter()
            .filter(|reading| !reading.device_id.trim().is_empty())
            .map(|reading| (reading.device_id.trim().to_string(), reading.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let previous_index = index(previous);
    let current_index = index(current);

    let mut filled = 0;
    for row in rows.iter_mut() {
        let Some(device_id) = device_of.get(&row.meter_id) else {
            continue;
        };
        let (Some(before), Some(after)) = (
            previous_index.get(device_id),
            current_index.get(device_id),
        ) else {
            continue;
        };
        let before_value = reading_value_for(before, row.tou_tier.as_deref());
        let after_value = reading_value_for(after, row.tou_tier.as_deref());
        // 供应商偶尔回传空串或 "--"，那种行按缺数据处理而不是填成 0。
        if before_value.trim().parse::<f64>().is_err() || after_value.trim().parse::<f64>().is_err()
        {
            continue;
        }
        row.previous_reading = before_value.trim().to_string();
        row.current_reading = after_value.trim().to_string();
        row.recalculate();
        filled += 1;
    }
    filled
}

pub(super) fn blank_utility_rows() -> Vec<UtilityDraft> {
    // 移动端只预置一条空明细，需要更多表计时由用户主动添加，避免出现六张空卡片。
    vec![UtilityDraft::default()]
}

pub(super) fn utility_total_cents(rows: &[UtilityDraft]) -> i64 {
    rows.iter()
        .filter(|row| !row.meter_name.trim().is_empty())
        .map(|row| parse_scaled_or_zero(&row.amount, CENTI_SCALE))
        .fold(0i64, i64::saturating_add)
}

pub(super) fn has_utility_rows(rows: &[UtilityDraft]) -> bool {
    rows.iter().any(|row| !row.meter_name.trim().is_empty())
}

pub(super) fn utility_inputs(
    rows: &[UtilityDraft],
    receipt_time: Option<spacetimedb_sdk::Timestamp>,
    label: &str,
) -> Result<Vec<UtilityBillInput>, String> {
    rows.iter()
        .filter(|row| !row.meter_name.trim().is_empty())
        .map(|row| {
            if row.meter_name.chars().count() > 120 {
                return Err(format!("{label}表计名称不能超过 120 个字符"));
            }
            if row.remark.chars().count() > 100 {
                return Err(format!("{label}明细备注不能超过 100 个字符"));
            }
            Ok(UtilityBillInput {
                meter_id: row.meter_id,
                tou_tier: row.tou_tier.clone(),
                meter_name: row.meter_name.trim().to_string(),
                previous_reading_centi: parse_scaled(
                    &row.previous_reading,
                    CENTI_SCALE,
                    &format!("{label}上月读数"),
                )?,
                current_reading_centi: parse_scaled(
                    &row.current_reading,
                    CENTI_SCALE,
                    &format!("{label}本月读数"),
                )?,
                monthly_usage_centi: parse_scaled(
                    &row.monthly_usage,
                    CENTI_SCALE,
                    &format!("{label}本月用量"),
                )?,
                multiplier_centi: parse_scaled(
                    &row.multiplier,
                    CENTI_SCALE,
                    &format!("{label}倍数"),
                )?,
                total_usage_centi: parse_scaled(
                    &row.total_usage,
                    CENTI_SCALE,
                    &format!("{label}实际用量"),
                )?,
                unit_price_scaled: parse_scaled(
                    &row.unit_price,
                    UNIT_PRICE_SCALE,
                    &format!("{label}单价"),
                )?,
                amount_cents: parse_scaled(&row.amount, CENTI_SCALE, &format!("{label}金额"))?,
                remark: (!row.remark.trim().is_empty()).then(|| row.remark.trim().to_string()),
                receipt_time,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct BankAccountDraft {
    pub name: String,
    pub number: String,
    pub bank: String,
}

impl BankAccountDraft {
    pub(super) fn from_stored(value: Option<&str>) -> Self {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::default();
        };
        serde_json::from_str(value).unwrap_or_else(|_| Self {
            number: value.to_string(),
            ..Self::default()
        })
    }

    /// 按「户名 · 开户行 · 账号」拼成一行展示文本，空字段自动跳过。
    ///
    /// 展示处不要直接打印库里的原值——那是一段 JSON，用户会看到
    /// `{"bank":"…","name":"…"}` 这样的原始字符串。
    pub(super) fn display_line(&self) -> Option<String> {
        let parts = [self.name.trim(), self.bank.trim(), self.number.trim()]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    pub(super) fn to_stored(&self, label: &str) -> Result<Option<String>, String> {
        if self.name.chars().count() > 120
            || self.number.chars().count() > 120
            || self.bank.chars().count() > 120
        {
            return Err(format!("{label}银行账户字段不能超过 120 个字符"));
        }
        if self.name.trim().is_empty()
            && self.number.trim().is_empty()
            && self.bank.trim().is_empty()
        {
            return Ok(None);
        }
        serde_json::to_string(&Self {
            name: self.name.trim().to_string(),
            number: self.number.trim().to_string(),
            bank: self.bank.trim().to_string(),
        })
            .map(Some)
            .map_err(|error| format!("序列化{label}银行账户失败：{error}"))
    }
}

#[component]
pub(super) fn UtilityWorksheet(
    kind: UtilityKind,
    mut rows: Signal<Vec<UtilityDraft>, SyncStorage>,
    readonly: bool,
    /// 所选合同约定的用表与单价。为空表示这份合同还没约定水电条款。
    #[props(default)]
    prefill: Vec<UtilityDraft>,
    /// 园区水电表台账，用来把明细行的 `meter_id` 翻成智能水电表设备号。
    #[props(default)]
    meters: Vec<UtilityMeter>,
) -> Element {
    let total = format_money_cents(utility_total_cents(&rows()));
    // 已经填了读数就不给一键覆盖的机会，否则一次误点就抹掉整月抄表结果。
    let prefill_count = prefill.len();
    let can_prefill = !readonly && prefill_count > 0 && !has_utility_rows(&rows());

    // 抄表日期默认「上月同日 → 今天」，与月结账期一致；用户可以改成实际抄表日。
    let mut current_date = use_signal(today);
    let mut previous_date = use_signal(|| previous_month_same_day(&today()));
    let mut fetching = use_signal(|| false);
    let mut fetch_notice = use_signal(|| None::<String>);
    let bound_rows = rows()
        .iter()
        .filter(|row| row.meter_id != 0)
        .count();
    let can_fetch = !readonly && bound_rows > 0;
    let meters = meters.clone();
    let reading_kind = match kind {
        UtilityKind::Electricity => MeterKind::Electric,
        UtilityKind::Water => MeterKind::Water,
    };

    // 明细行一带出来就自动去智能水电表接口拉读数，不用再点一次。
    //
    // 表台账从 context 现取而不是用 `meters` 这个 prop：prop 是首次渲染时的快照，
    // 而订阅推送可能晚于首次渲染，用快照会拿到空列表、一行都对不上。
    //
    // 只自动一次，失败也不重试——按钮仍然留着，网络或供应商接口的问题让用户自己
    // 决定什么时候重来，比在后台反复打对方接口好。
    let workspace = use_context::<crate::state::WorkspaceState>();
    let mut auto_fetched = use_signal(|| false);
    use_effect(move || {
        if readonly || auto_fetched() {
            return;
        }
        let current = rows();
        // 判据是「有没有填过读数」，不能用 `has_utility_rows`——那是「有没有填表名」，
        // 而刚从合同带出来的行本来就有表名，用它会把自动拉读数挡在门外。
        let already_read = current
            .iter()
            .any(|row| !row.current_reading.trim().is_empty());
        if !current.iter().any(|row| row.meter_id != 0) || already_read {
            return;
        }
        let Some(token) = load_saved_token() else {
            return;
        };
        auto_fetched.set(true);
        let before = previous_date();
        let after = current_date();
        let meters = (workspace.utility_meters)();
        fetching.set(true);
        spawn(async move {
            let previous = load_smart_meter_readings(token.clone(), reading_kind, before).await;
            let current = load_smart_meter_readings(token, reading_kind, after).await;
            fetching.set(false);
            match (previous, current) {
                (Ok(previous), Ok(current)) => {
                    let mut next = rows();
                    let filled =
                        fill_readings(&mut next, &meters, &previous.readings, &current.readings);
                    rows.set(next);
                    if filled > 0 {
                        fetch_notice
                            .set(Some(format!("已自动带入 {filled} 行读数，用量与金额已算好")));
                    }
                }
                (Err(error), _) | (_, Err(error)) => {
                    fetch_notice.set(Some(format!(
                        "自动读取智能水电表数据失败：{error}。确认抄表日期后可点按钮重试。"
                    )));
                }
            }
        });
    });

    rsx! {
        section { class: "subsection",
            div { class: "section-header",
                div { class: "stack-tight",
                    h4 { "{kind.title()}明细" }
                    small { class: "hint", "逐个表计录入，差值、实际用量和金额自动计算" }
                }
                div { class: "section-tools",
                    if can_prefill {
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            r#type: "button",
                            onclick: move |_| rows.set(prefill.clone()),
                            "按合同带入 {prefill_count} 项"
                        }
                    }
                    Badge { variant: BadgeVariant::Secondary, "本项合计 ¥{total}" }
                }
            }
            if can_fetch {
                // 用布局层已有的 `.filters` 而不是自己编一套：账单表单里「收款时间 +
                // 同步今日」就是这个结构，两处长得一样才不会看出是两拨人做的。
                div { class: "filters reading-fetch",
                    div { class: "field",
                        span { class: "field-label", "上期抄表日" }
                        // 用组件库的 DateField 而不是裸 `<input type="date">`：后者由浏览器
                        // 按系统语言排版，中文环境下会显示成 `06/30/2026`，和页面上其他
                        // 日期（`2026-06-30`）对不上。
                        DateField {
                            value: previous_date(),
                            on_change: move |value: String| previous_date.set(value),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "本期抄表日" }
                        DateField {
                            value: current_date(),
                            on_change: move |value: String| current_date.set(value),
                        }
                    }
                    div { class: "field is-action",
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            disabled: fetching(),
                            onclick: move |_| {
                                if fetching() { return; }
                                let Some(token) = load_saved_token() else {
                                    fetch_notice.set(Some("登录凭证不存在，请重新登录".into()));
                                    return;
                                };
                                let before = previous_date();
                                let after = current_date();
                                let meters = meters.clone();
                                fetch_notice.set(None);
                                fetching.set(true);
                                spawn(async move {
                                    let previous = load_smart_meter_readings(token.clone(), reading_kind, before).await;
                                    let current = load_smart_meter_readings(token, reading_kind, after).await;
                                    fetching.set(false);
                                    match (previous, current) {
                                        (Ok(previous), Ok(current)) => {
                                            let mut next = rows();
                                            let filled = fill_readings(&mut next, &meters, &previous.readings, &current.readings);
                                            rows.set(next);
                                            fetch_notice.set(Some(if filled == 0 {
                                                "这两个日期没有匹配到已绑定设备的读数，请确认抄表日期和设备绑定".into()
                                            } else {
                                                format!("已带入 {filled} 行读数，用量与金额已按公式重算")
                                            }));
                                        }
                                        (Err(error), _) | (_, Err(error)) => {
                                            fetch_notice.set(Some(format!("读取智能水电表数据失败：{error}")));
                                        }
                                    }
                                });
                            },
                            if fetching() { "读取中…" } else { "从智能水电表带入读数" }
                        }
                    }
                }
                // 「N 行已绑定设备」不能塞进按钮那一格：`.filters` 是底部对齐，
                // 格子里叠两行的话贴着行底的是这句说明，按钮反而被顶到输入框上面
                // 去，和日期选择器错开半行。放到整行下面，按钮就和输入框齐平了。
                small { class: "hint", "{bound_rows} 行已绑定设备" }
                if let Some(message) = fetch_notice() {
                    p { class: "notice", role: "status", "{message}" }
                }
            }
            div { class: "stack",
                for (index , row) in rows().into_iter().enumerate() {
                    div { class: "panel is-plain", key: "utility-{index}",
                        div { class: "section-header",
                            strong { "明细 {index + 1:02}" }
                            if !readonly {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    class: "is-quiet-danger",
                                    size: ButtonSize::Sm,
                                    r#type: "button",
                                    aria_label: "删除第 {index + 1} 条明细",
                                    // 至少保留一条，否则表单会没有任何可填的明细行
                                    disabled: rows().len() <= 1,
                                    onclick: move |_| {
                                        rows.with_mut(|items| {
                                            if items.len() > 1 {
                                                items.remove(index);
                                            }
                                        })
                                    },
                                    "移除"
                                }
                            }
                        }
                        div { class: "form-grid",
                            div { class: "field is-wide",
                                span { class: "field-label", "表计名称" }
                                Input {
                                    value: row.meter_name,
                                    readonly,
                                    placeholder: "例如：8303 电表",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.meter_name = event.value();
                                            }
                                        })
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "{kind.previous_label()}" }
                                Input {
                                    inputmode: "decimal",
                                    value: row.previous_reading,
                                    readonly,
                                    placeholder: "0",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.previous_reading = event.value();
                                                row.recalculate();
                                            }
                                        })
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "本月抄表数" }
                                Input {
                                    inputmode: "decimal",
                                    value: row.current_reading,
                                    readonly,
                                    placeholder: "0",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.current_reading = event.value();
                                                row.recalculate();
                                            }
                                        })
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "倍数" }
                                Input {
                                    inputmode: "decimal",
                                    value: row.multiplier,
                                    readonly,
                                    placeholder: "1",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.multiplier = event.value();
                                                row.recalculate();
                                            }
                                        })
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "{kind.price_label()}" }
                                Input {
                                    inputmode: "decimal",
                                    value: row.unit_price,
                                    readonly,
                                    placeholder: "0.00",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.unit_price = event.value();
                                                row.recalculate();
                                            }
                                        })
                                    },
                                }
                            }
                            div { class: "field",
                                span { class: "field-label", "{kind.usage_label()}" }
                                span { class: "field-static is-mono", "{row.monthly_usage}" }
                            }
                            div { class: "field",
                                span { class: "field-label", "{kind.total_usage_label()}" }
                                span { class: "field-static is-mono", "{row.total_usage}" }
                            }
                            div { class: "field",
                                span { class: "field-label", "{kind.amount_label()}" }
                                span { class: "field-static is-mono", "¥{row.amount}" }
                            }
                            div { class: "field is-wide",
                                span { class: "field-label", "备注" }
                                Input {
                                    value: row.remark,
                                    readonly,
                                    placeholder: "可选：填写表计或计费说明",
                                    oninput: move |event: FormEvent| {
                                        rows.with_mut(|items| {
                                            if let Some(row) = items.get_mut(index) {
                                                row.remark = event.value();
                                            }
                                        })
                                    },
                                }
                            }
                        }
                    }
                }
            }
            if !readonly {
                div { class: "row",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| rows.write().push(UtilityDraft::default()),
                        "添加一条{kind.title()}明细"
                    }
                }
            }
        }
    }
}
fn parse_scaled_or_zero(value: &str, scale: i64) -> i64 {
    parse_scaled(value, scale, "数值").unwrap_or_default()
}

fn parse_scaled(value: &str, scale: i64, label: &str) -> Result<i64, String> {
    let value = value
        .trim()
        .replace(',', "")
        .replace('¥', "")
        .replace('￥', "");
    if value.is_empty() {
        return Ok(0);
    }
    if value.starts_with('-') {
        return Err(format!("{label}不能为负数"));
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((&value, ""));
    if whole.is_empty() || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{label}格式不正确"));
    }
    let precision = scale.ilog10() as usize;
    if fraction.len() > precision || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{label}最多保留 {precision} 位小数"));
    }
    let whole = whole
        .parse::<i64>()
        .map_err(|_| format!("{label}数值过大"))?;
    let mut fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i64>()
            .map_err(|_| format!("{label}格式不正确"))?
    };
    for _ in fraction.len()..precision {
        fraction_value = fraction_value.saturating_mul(10);
    }
    whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction_value))
        .ok_or_else(|| format!("{label}数值过大"))
}

fn format_scaled(value: i64, scale: i64) -> String {
    let precision = scale.ilog10() as usize;
    let whole = value / scale;
    let fraction = (value % scale).abs();
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:0precision$}")
        .trim_end_matches('0')
        .to_string();
    format!("{whole}.{fraction}")
}

fn format_money_cents(value: i64) -> String {
    format!("{}.{:02}", value / 100, (value % 100).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter(id: u64, code: &str, electric: bool, tou: bool) -> UtilityMeter {
        UtilityMeter {
            meter_id: id,
            customer_id: String::new(),
            park_id: 1,
            meter_code: code.into(),
            is_electric: electric,
            factory_floor_id: 0,
            dormitory_floor_id: 0,
            multiplier_centi: 100,
            is_time_of_use: tou,
            external_device_id: None,
            remark: None,
            is_deleted: false,
            created_at: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
        }
    }

    fn link(meter_id: u64, unit: Option<i64>, tiers: Option<[i64; 4]>) -> RentalTenantMeter {
        RentalTenantMeter {
            id: 0,
            customer_id: String::new(),
            rental_tenant_id: 1,
            meter_id,
            unit_price_scaled: unit,
            price_tip_scaled: tiers.map(|t| t[0]),
            price_peak_scaled: tiers.map(|t| t[1]),
            price_flat_scaled: tiers.map(|t| t[2]),
            price_valley_scaled: tiers.map(|t| t[3]),
            remark: None,
            created_at: spacetimedb_sdk::Timestamp::from_micros_since_unix_epoch(0),
            updated_at: None,
        }
    }

    #[test]
    fn 分时表按时段拆成四行() {
        let meters = vec![meter(1, "A1", true, true)];
        let links = vec![link(1, None, Some([150_000_000, 120_000_000, 90_000_000, 50_000_000]))];
        let rows = drafts_from_contract(UtilityKind::Electricity, &meters, &links);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].meter_name, "A1 · 尖");
        assert_eq!(rows[3].meter_name, "A1 · 谷");
        assert_eq!(rows[0].unit_price, "1.5");
        assert_eq!(rows[3].unit_price, "0.5");
        // 读数留空等抄表，倍率从表上带出。
        assert_eq!(rows[0].previous_reading, "");
        assert_eq!(rows[0].multiplier, "1");
    }

    #[test]
    fn 水电表各归各的工作表() {
        let meters = vec![meter(1, "电1", true, false), meter(2, "水1", false, false)];
        let links = vec![link(1, Some(105_000_000), None), link(2, Some(320_000_000), None)];
        let ele = drafts_from_contract(UtilityKind::Electricity, &meters, &links);
        let water = drafts_from_contract(UtilityKind::Water, &meters, &links);
        assert_eq!(ele.len(), 1);
        assert_eq!(ele[0].meter_name, "电1");
        assert_eq!(ele[0].unit_price, "1.05");
        assert_eq!(water.len(), 1);
        assert_eq!(water[0].meter_name, "水1");
    }

    #[test]
    fn 没约定单价的关联不生成明细行() {
        // 服务端会拦住这种数据，但已删除的表和历史脏数据仍可能漏进来，
        // 生成一行单价为空的明细只会让账单金额算成零。
        let meters = vec![meter(1, "A1", true, false)];
        assert!(drafts_from_contract(UtilityKind::Electricity, &meters, &[link(1, None, None)]).is_empty());
        // 表已归档时同样跳过。
        let mut archived = meters.clone();
        archived[0].is_deleted = true;
        assert!(drafts_from_contract(UtilityKind::Electricity, &archived, &[link(1, Some(1), None)]).is_empty());
    }

    #[test]
    fn 水电公式按原表格关系计算() {
        let mut row = UtilityDraft {
            previous_reading: "100".into(),
            current_reading: "125.5".into(),
            multiplier: "2".into(),
            unit_price: "0.8".into(),
            ..UtilityDraft::default()
        };
        row.recalculate();
        assert_eq!(row.monthly_usage, "25.5");
        assert_eq!(row.total_usage, "51");
        assert_eq!(row.amount, "40.80");
    }

    #[test]
    fn 银行账户展示不会露出原始_json() {
        let stored =
            r#"{"bank":"中国民生银行深圳红山支行","name":"喻必胜","number":"6226 2206 3587 2558"}"#;
        let draft = BankAccountDraft::from_stored(Some(stored));
        assert_eq!(
            draft.display_line().as_deref(),
            Some("喻必胜 · 中国民生银行深圳红山支行 · 6226 2206 3587 2558"),
        );
    }

    #[test]
    fn 银行账户展示跳过空字段() {
        let draft = BankAccountDraft {
            name: String::new(),
            number: "6226".into(),
            bank: String::new(),
        };
        assert_eq!(draft.display_line().as_deref(), Some("6226"));
        assert!(BankAccountDraft::default().display_line().is_none());
    }

    #[test]
    fn 银行账户兼容旧纯文本并保存为_json() {
        let account = BankAccountDraft::from_stored(Some("62220001"));
        assert_eq!(account.number, "62220001");
        assert!(account
            .to_stored("对私")
            .unwrap()
            .unwrap()
            .contains("62220001"));
    }

    fn reading(device_id: &str, total: &str, tiers: [&str; 4]) -> SmartMeterReading {
        SmartMeterReading {
            room_id: String::new(),
            room_name: String::new(),
            device_id: device_id.into(),
            com_address: String::new(),
            data_item_name: String::new(),
            data_value: total.into(),
            data_value_tip: tiers[0].into(),
            data_value_peak: tiers[1].into(),
            data_value_flat: tiers[2].into(),
            data_value_valley: tiers[3].into(),
            freeze_time: String::new(),
            write_time: String::new(),
        }
    }

    fn bound(id: u64, code: &str, device_id: &str, tou: bool) -> UtilityMeter {
        let mut row = meter(id, code, true, tou);
        row.external_device_id = Some(device_id.into());
        row
    }

    #[test]
    fn 读数按设备号填进对应明细行并重算用量() {
        let meters = vec![bound(7, "A1", "00998", false)];
        let mut rows = vec![UtilityDraft {
            meter_id: 7,
            meter_name: "A1".into(),
            multiplier: "1".into(),
            unit_price: "0.8".into(),
            ..Default::default()
        }];
        let filled = fill_readings(
            &mut rows,
            &meters,
            &[reading("00998", "100", ["0", "0", "0", "0"])],
            &[reading("00998", "150", ["0", "0", "0", "0"])],
        );
        assert_eq!(filled, 1);
        assert_eq!(rows[0].previous_reading, "100");
        assert_eq!(rows[0].current_reading, "150");
        assert_eq!(rows[0].monthly_usage, "50");
        assert_eq!(rows[0].amount, "40.00");
    }

    #[test]
    fn 分时四行各读各自那一段() {
        // 读错段的后果是四行填成同一个数，用量全变 0。
        let meters = vec![bound(7, "A1", "00998", true)];
        let mut rows = vec![
            UtilityDraft { meter_id: 7, tou_tier: Some(TOU_TIP.into()), meter_name: "A1 · 尖".into(), ..Default::default() },
            UtilityDraft { meter_id: 7, tou_tier: Some(TOU_PEAK.into()), meter_name: "A1 · 峰".into(), ..Default::default() },
            UtilityDraft { meter_id: 7, tou_tier: Some(TOU_FLAT.into()), meter_name: "A1 · 平".into(), ..Default::default() },
            UtilityDraft { meter_id: 7, tou_tier: Some(TOU_VALLEY.into()), meter_name: "A1 · 谷".into(), ..Default::default() },
        ];
        let filled = fill_readings(
            &mut rows,
            &meters,
            &[reading("00998", "400", ["10", "20", "30", "40"])],
            &[reading("00998", "480", ["11", "22", "33", "44"])],
        );
        assert_eq!(filled, 4);
        assert_eq!(
            rows.iter().map(|row| row.previous_reading.as_str()).collect::<Vec<_>>(),
            vec!["10", "20", "30", "40"]
        );
        assert_eq!(
            rows.iter().map(|row| row.current_reading.as_str()).collect::<Vec<_>>(),
            vec!["11", "22", "33", "44"]
        );
    }

    #[test]
    fn 未绑定设备的行保持原样() {
        // 手抄表不能因为点了「一键带入」就被抹掉。
        let meters = vec![meter(7, "A1", true, false)];
        let mut rows = vec![UtilityDraft {
            meter_id: 7,
            meter_name: "A1".into(),
            previous_reading: "88".into(),
            current_reading: "99".into(),
            ..Default::default()
        }];
        assert_eq!(fill_readings(&mut rows, &meters, &[], &[]), 0);
        assert_eq!(rows[0].previous_reading, "88");
        assert_eq!(rows[0].current_reading, "99");
    }

    #[test]
    fn 只有一个日期有读数时不填() {
        // 只有本期没有上期，差值会把整个累计值当成本月用量。
        let meters = vec![bound(7, "A1", "00998", false)];
        let mut rows = vec![UtilityDraft { meter_id: 7, meter_name: "A1".into(), ..Default::default() }];
        let filled = fill_readings(
            &mut rows,
            &meters,
            &[],
            &[reading("00998", "150", ["0", "0", "0", "0"])],
        );
        assert_eq!(filled, 0);
        assert!(rows[0].current_reading.is_empty());
    }

    #[test]
    fn 供应商回传非数字时按缺数据处理() {
        let meters = vec![bound(7, "A1", "00998", false)];
        let mut rows = vec![UtilityDraft { meter_id: 7, meter_name: "A1".into(), ..Default::default() }];
        let filled = fill_readings(
            &mut rows,
            &meters,
            &[reading("00998", "--", ["", "", "", ""])],
            &[reading("00998", "150", ["", "", "", ""])],
        );
        assert_eq!(filled, 0, "「--」不能被当成 0 填进账单");
    }

    #[test]
    fn 已注销的表不参与读数带入() {
        let mut row = bound(7, "A1", "00998", false);
        row.is_deleted = true;
        let mut rows = vec![UtilityDraft { meter_id: 7, meter_name: "A1".into(), ..Default::default() }];
        let filled = fill_readings(
            &mut rows,
            &[row],
            &[reading("00998", "100", ["0", "0", "0", "0"])],
            &[reading("00998", "150", ["0", "0", "0", "0"])],
        );
        assert_eq!(filled, 0);
    }

    /// 「有表名」和「填过读数」是两件事，自动拉读数只能看后者。
    ///
    /// 这两个判据混用过一次：刚从合同带出来的明细行本来就有表名，用
    /// `has_utility_rows` 当「已经抄过表」的判据，会让自动拉读数一次都不触发，
    /// 而且界面上完全看不出来——表和单价都在，只是读数永远空着。
    #[test]
    fn 从合同带出来的行有表名但还没有读数() {
        let meters = vec![meter(7, "A1", true, false)];
        let links = vec![link(7, Some(100_000_000), None)];
        let rows = drafts_from_contract(UtilityKind::Electricity, &meters, &links);
        assert!(has_utility_rows(&rows), "带出来的行应当已经有表名");
        assert!(
            rows.iter().all(|row| row.current_reading.trim().is_empty()),
            "但一行读数都不该有"
        );
    }

    #[test]
    fn 合同带入的明细带上表主键与分时标识() {
        // 认表靠 meter_id 和 tou_tier，不再靠表名文本。
        let meters = vec![meter(7, "A1", true, true)];
        let links = vec![link(7, None, Some([10, 20, 30, 40]))];
        let rows = drafts_from_contract(UtilityKind::Electricity, &meters, &links);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| row.meter_id == 7));
        assert_eq!(
            rows.iter().map(|row| row.tou_tier.as_deref().unwrap()).collect::<Vec<_>>(),
            vec![TOU_TIP, TOU_PEAK, TOU_FLAT, TOU_VALLEY]
        );
        assert_eq!(rows[0].meter_name, "A1 · 尖");
    }
}
