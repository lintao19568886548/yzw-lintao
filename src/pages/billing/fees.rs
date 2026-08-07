//! 按合同约定算出这一期该收的电损费、服务费、垃圾费和基本电费。
//!
//! 这里全是纯函数，输入是「已经填好的水电明细 + 表台账 + 合同约定」，输出是金额。
//! 单独成模块而不是塞进 `worksheet.rs`，是因为算错的后果是每个月多收或少收租户的
//! 钱，而这类错误在界面上看不出来——只能靠测试。
//!
//! 与服务端 `reducers::rental::tenant_fee` 是同一套算法。客户端算是为了让用户在
//! 保存之前就看到金额，服务端存的是客户端算完提交上来的数——两边口径必须一致，
//! 所以基数定义和取整方向都照抄，测试也覆盖同样的边界。

use std::collections::BTreeMap;

use crate::spacetime_bindings::{
    rental_tenant_fee_type::RentalTenantFee, utility_meter_type::UtilityMeter,
};

use super::worksheet::UtilityDraft;

/// 一块表装在哪里。基数里的「厂房」「宿舍」按这个分，不需要用户逐条选。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeterScene {
    Factory,
    Dormitory,
    /// 既不挂厂房层也不挂宿舍层，即园区公共区域的表。
    Public,
}

/// 从表台账推断使用场景。
///
/// 老系统要求每条抄表记录手选「厂房／宿舍」，选错就算错，而且同一块表这个月选
/// 厂房、下个月选宿舍也没人拦。这里改为从安装位置算出来——表装在哪一层是台账里
/// 的事实，不是每月要重新回答的问题。
pub(super) fn meter_scene(meter: &UtilityMeter) -> MeterScene {
    if meter.factory_floor_id != 0 {
        MeterScene::Factory
    } else if meter.dormitory_floor_id != 0 {
        MeterScene::Dormitory
    } else {
        MeterScene::Public
    }
}

/// 比例项的各个计费基数。单位见各字段——度和钱混在一起是故意的，见服务端同名结构。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FeeBases {
    /// 厂房电表的计费用量合计，度乘以一百。
    pub factory_usage_centi: i64,
    /// 厂房电表的电费合计，分。
    pub factory_fee_cents: i64,
    /// 宿舍电表的电费合计，分。
    pub dormitory_fee_cents: i64,
    /// 全部电表的电费合计（含公共区域），分。
    pub total_ele_fee_cents: i64,
    /// 基本电费，分。
    pub basic_ele_fee_cents: i64,
}

impl FeeBases {
    fn value(&self, rate_base: &str) -> Option<i64> {
        Some(match rate_base {
            "factory_usage" => self.factory_usage_centi,
            "factory_fee" => self.factory_fee_cents,
            "factory_fee_with_basic" => {
                self.factory_fee_cents.saturating_add(self.basic_ele_fee_cents)
            }
            "factory_dorm_fee" => self.factory_fee_cents.saturating_add(self.dormitory_fee_cents),
            "total_ele_fee" => self.total_ele_fee_cents,
            _ => return None,
        })
    }
}

/// 把当期电费明细汇总成各个基数。
///
/// 只看电费明细：水费不参与电损和服务费的计算基数——老系统那五种基数全部是电。
pub(super) fn collect_bases(
    ele_rows: &[UtilityDraft],
    meters: &[UtilityMeter],
    basic_ele_fee_cents: i64,
) -> FeeBases {
    let by_id = meters
        .iter()
        .map(|meter| (meter.meter_id, meter))
        .collect::<BTreeMap<_, _>>();
    let mut bases = FeeBases {
        basic_ele_fee_cents,
        ..Default::default()
    };
    for row in ele_rows {
        let usage = parse_centi(&row.total_usage);
        let amount = parse_cents(&row.amount);
        bases.total_ele_fee_cents = bases.total_ele_fee_cents.saturating_add(amount);
        // 手工补录的行（meter_id 为 0）算进总数，但没有安装位置，进不了厂房或
        // 宿舍这两个桶——它们的场景确实无从判断，硬塞进厂房会让电损凭空变大。
        match by_id.get(&row.meter_id).map(|meter| meter_scene(meter)) {
            Some(MeterScene::Factory) => {
                bases.factory_usage_centi = bases.factory_usage_centi.saturating_add(usage);
                bases.factory_fee_cents = bases.factory_fee_cents.saturating_add(amount);
            }
            Some(MeterScene::Dormitory) => {
                bases.dormitory_fee_cents = bases.dormitory_fee_cents.saturating_add(amount);
            }
            _ => {}
        }
    }
    bases
}

/// 基本电费 = 计费容量 × 单价。两项任一为空即不收。
///
/// 容量乘一百、单价乘一亿；结果要分，所以再乘一百，正好和容量的百倍抵消，
/// 除数就是一亿。
pub(super) fn basic_ele_fee_cents(
    capacity_centi_kw: Option<i64>,
    price_scaled: Option<i64>,
) -> i64 {
    let (Some(capacity), Some(price)) = (capacity_centi_kw, price_scaled) else {
        return 0;
    };
    ((capacity as i128 * price as i128) / 100_000_000) as i64
}

/// 一项约定费用当期该收多少分。
///
/// 向零取整，也就是**永远不会多收**——多收一分钱要解释起来比少收一分麻烦得多。
pub(super) fn basic_ele_note(
    capacity_centi_kw: Option<i64>,
    price_scaled: Option<i64>,
) -> Option<String> {
    let (capacity, price) = (capacity_centi_kw?, price_scaled?);
    Some(format!(
        "{} kW × {} 元/kW",
        trim_zeros(capacity, 2),
        trim_zeros(price, 8)
    ))
}

/// 定点整数转成去掉尾随零的十进制文本。`23.00000000` 显示成 `23`。
fn trim_zeros(value: i64, digits: u32) -> String {
    let scale = 10_i64.pow(digits);
    let text = format!(
        "{}.{:0width$}",
        value / scale,
        (value % scale).abs(),
        width = digits as usize
    );
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

/// 一项约定费用当期该收多少分。
///
/// 向零取整，也就是**永远不会多收**——多收一分钱要解释起来比少收一分麻烦得多。
pub(super) fn fee_amount_cents(fee: &RentalTenantFee, bases: &FeeBases) -> Result<i64, String> {
    match fee.charge_mode.as_str() {
        "fixed" => Ok(fee.amount_cents.unwrap_or(0)),
        "rate" => {
            let rate = fee.rate_basis_points.unwrap_or(0);
            let base_name = fee
                .rate_base
                .as_deref()
                .ok_or("按比例计费的费用没有指定计费基数")?;
            let base = bases
                .value(base_name)
                .ok_or_else(|| format!("不认识的计费基数「{base_name}」"))?;
            Ok(((base as i128 * rate as i128) / 10_000) as i64)
        }
        other => Err(format!("不认识的计费方式「{other}」")),
    }
}

/// 按合同约定算出这一期的三项费用，键是 `fee_kind`。
///
/// 算不出来的项直接报错而不是跳过：静默跳过就是少收一笔钱，没有人会发现。
pub(super) fn contract_fee_amounts(
    fees: &[RentalTenantFee],
    bases: &FeeBases,
) -> Result<BTreeMap<String, i64>, String> {
    let mut amounts = BTreeMap::new();
    for fee in fees {
        amounts.insert(fee.fee_kind.clone(), fee_amount_cents(fee, bases)?);
    }
    Ok(amounts)
}

fn parse_centi(value: &str) -> i64 {
    parse_scaled(value, 100)
}

fn parse_cents(value: &str) -> i64 {
    parse_scaled(value, 100)
}

/// 把「12.34」这样的十进制文本按给定倍数转成整数，解析不了当零。
///
/// 当零而不是报错：这些文本来自用户还在编辑的输入框，中间态（空串、只有一个
/// 小数点）是正常的，不该让整张表算不出数。真正的校验在提交时做。
fn parse_scaled(value: &str, scale: i64) -> i64 {
    let text = value.trim().replace(',', "");
    if text.is_empty() {
        return 0;
    }
    let (int_part, frac_part) = match text.split_once('.') {
        Some((head, tail)) => (head, tail),
        None => (text.as_str(), ""),
    };
    let digits = scale.ilog10() as usize;
    let mut frac = frac_part.chars().filter(char::is_ascii_digit).collect::<String>();
    frac.truncate(digits);
    while frac.len() < digits {
        frac.push('0');
    }
    let int_value = int_part.parse::<i64>().unwrap_or(0);
    let frac_value = frac.parse::<i64>().unwrap_or(0);
    let sign = if int_part.trim_start().starts_with('-') { -1 } else { 1 };
    int_value
        .saturating_mul(scale)
        .saturating_add(sign * frac_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacetimedb_sdk::Timestamp;

    fn meter(meter_id: u64, factory_floor_id: u64, dormitory_floor_id: u64) -> UtilityMeter {
        UtilityMeter {
            meter_id,
            customer_id: "public".into(),
            park_id: 1,
            meter_code: format!("M{meter_id}"),
            is_electric: true,
            factory_floor_id,
            dormitory_floor_id,
            multiplier_centi: 100,
            is_time_of_use: false,
            external_device_id: None,
            remark: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn row(meter_id: u64, total_usage: &str, amount: &str) -> UtilityDraft {
        UtilityDraft {
            meter_id,
            total_usage: total_usage.into(),
            amount: amount.into(),
            ..Default::default()
        }
    }

    fn fee(kind: &str, mode: &str, amount: Option<i64>, rate: Option<i64>, base: Option<&str>) -> RentalTenantFee {
        RentalTenantFee {
            id: 1,
            customer_id: "public".into(),
            rental_tenant_id: 1,
            fee_kind: kind.into(),
            charge_mode: mode.into(),
            amount_cents: amount,
            rate_basis_points: rate,
            rate_base: base.map(str::to_string),
            remark: None,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 使用场景按安装位置推断而不是手选() {
        assert_eq!(meter_scene(&meter(1, 5, 0)), MeterScene::Factory);
        assert_eq!(meter_scene(&meter(2, 0, 7)), MeterScene::Dormitory);
        assert_eq!(meter_scene(&meter(3, 0, 0)), MeterScene::Public);
    }

    #[test]
    fn 基数按场景分桶且总数含公共区域() {
        let meters = vec![meter(1, 5, 0), meter(2, 0, 7), meter(3, 0, 0)];
        let rows = vec![
            row(1, "1000", "1000.00"), // 厂房
            row(2, "300", "300.00"),   // 宿舍
            row(3, "200", "200.00"),   // 公共
        ];
        let bases = collect_bases(&rows, &meters, 20_000);
        assert_eq!(bases.factory_usage_centi, 100_000);
        assert_eq!(bases.factory_fee_cents, 100_000);
        assert_eq!(bases.dormitory_fee_cents, 30_000);
        // 公共区域只进总数，不进厂房也不进宿舍。
        assert_eq!(bases.total_ele_fee_cents, 150_000);
        assert_eq!(bases.basic_ele_fee_cents, 20_000);
    }

    #[test]
    fn 手工补录的行只进总数() {
        // meter_id 为 0 的行没有安装位置，硬塞进厂房会让电损凭空变大。
        let meters = vec![meter(1, 5, 0)];
        let rows = vec![row(1, "1000", "1000.00"), row(0, "500", "500.00")];
        let bases = collect_bases(&rows, &meters, 0);
        assert_eq!(bases.factory_fee_cents, 100_000);
        assert_eq!(bases.total_ele_fee_cents, 150_000);
    }

    #[test]
    fn 各个基数各按各的算() {
        let bases = FeeBases {
            factory_usage_centi: 100_000,
            factory_fee_cents: 100_000,
            dormitory_fee_cents: 30_000,
            total_ele_fee_cents: 150_000,
            basic_ele_fee_cents: 20_000,
        };
        let cases = [
            ("factory_fee", 5_000),            // 5% × 1000 元
            ("factory_fee_with_basic", 6_000), // 5% × 1200 元
            ("factory_dorm_fee", 6_500),       // 5% × 1300 元
            ("total_ele_fee", 7_500),          // 5% × 1500 元
            ("factory_usage", 5_000),          // 5% × 1000 度 = 50 度（单位是度）
        ];
        for (base, expected) in cases {
            let row = fee("loss", "rate", None, Some(500), Some(base));
            assert_eq!(fee_amount_cents(&row, &bases), Ok(expected), "基数 {base}");
        }
    }

    #[test]
    fn 固定金额不看基数() {
        let row = fee("service", "fixed", Some(80_000), None, None);
        assert_eq!(fee_amount_cents(&row, &FeeBases::default()), Ok(80_000));
    }

    #[test]
    fn 除不尽时向零取整不多收() {
        let bases = FeeBases { factory_fee_cents: 100_000, ..Default::default() };
        // 3.33% × 1000 元 = 33.30 元，不进位到 33.31。
        let row = fee("loss", "rate", None, Some(333), Some("factory_fee"));
        assert_eq!(fee_amount_cents(&row, &bases), Ok(3_330));
    }

    #[test]
    fn 基数不认识时报错而不是当成零() {
        // 当成零就是静默少收，没人会发现。
        let row = fee("loss", "rate", None, Some(500), Some("厂房电费"));
        assert!(fee_amount_cents(&row, &FeeBases::default()).is_err());
    }

    #[test]
    fn 基本电费按容量乘单价() {
        // 500 kW × 23 元/kW = 11500 元
        assert_eq!(basic_ele_fee_cents(Some(50_000), Some(23 * 100_000_000)), 1_150_000);
        // 123.45 kW × 23 元/kW = 2839.35 元
        assert_eq!(basic_ele_fee_cents(Some(12_345), Some(23 * 100_000_000)), 283_935);
        // 缺一项就不收
        assert_eq!(basic_ele_fee_cents(None, Some(23 * 100_000_000)), 0);
        assert_eq!(basic_ele_fee_cents(Some(50_000), None), 0);
    }

    #[test]
    fn 三项费用一次算齐() {
        let bases = FeeBases {
            factory_fee_cents: 100_000,
            total_ele_fee_cents: 150_000,
            ..Default::default()
        };
        let fees = vec![
            fee("loss", "rate", None, Some(500), Some("factory_fee")),
            fee("service", "fixed", Some(80_000), None, None),
            fee("garbage", "rate", None, Some(100), Some("total_ele_fee")),
        ];
        let amounts = contract_fee_amounts(&fees, &bases).expect("三项都算得出来");
        assert_eq!(amounts.get("loss"), Some(&5_000));
        assert_eq!(amounts.get("service"), Some(&80_000));
        assert_eq!(amounts.get("garbage"), Some(&1_500));
    }

    #[test]
    fn 输入框的中间态当零而不是让整张表算不出数() {
        assert_eq!(parse_scaled("", 100), 0);
    }

    #[test]
    fn 基本电费的算式说明去掉尾随零() {
        // 23.00000000 元/kW 显示成 23，而不是一串零。
        assert_eq!(
            basic_ele_note(Some(50_000), Some(23 * 100_000_000)).as_deref(),
            Some("500 kW × 23 元/kW")
        );
        assert_eq!(
            basic_ele_note(Some(12_345), Some(23_500_000_00)).as_deref(),
            Some("123.45 kW × 23.5 元/kW")
        );
        // 条款不全就没有算式可说明。
        assert_eq!(basic_ele_note(None, Some(1)), None);
        assert_eq!(basic_ele_note(Some(1), None), None);
    }

    #[test]
    fn 十进制文本解析覆盖各种中间态() {
        assert_eq!(parse_scaled("", 100), 0);
        assert_eq!(parse_scaled(".", 100), 0);
        assert_eq!(parse_scaled("12", 100), 1_200);
        assert_eq!(parse_scaled("12.3", 100), 1_230);
        assert_eq!(parse_scaled("12.34", 100), 1_234);
        // 多余小数位截断而不是四舍五入，与金额口径一致。
        assert_eq!(parse_scaled("12.349", 100), 1_234);
        assert_eq!(parse_scaled("1,200.50", 100), 120_050);
    }
}
