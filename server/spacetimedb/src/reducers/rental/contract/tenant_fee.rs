//! 合同约定费用（电损／服务／垃圾）的维护与计算。
//!
//! 语义与用表关系一致：整体替换。一次调用给出这份合同约定的全部费用，服务端
//! 删旧建新。
//!
//! 金额计算是纯函数 [`fee_amount_cents`]，输入是一组已经算好的账单基数。把它和
//! 数据库读写分开，是因为"5% 该乘在哪个数上"是这里唯一会算错的地方，而算错的
//! 后果是每个月多收或少收租户的钱——这种逻辑必须能单独测。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{require_rental_tenant, AdminContext},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 合同约定的一项周期性费用。
#[derive(SpacetimeType)]
pub struct RentalTenantFeeInput {
    pub fee_kind: String,
    pub charge_mode: String,
    pub amount_cents: Option<i64>,
    pub rate_basis_points: Option<i64>,
    pub rate_base: Option<String>,
    pub remark: Option<String>,
}

/// 比例项能选的计费基数，全部由当期水电明细算出。
///
/// 单位不统一是故意的：`factory_usage` 是度数（乘一百），其余四项是金额（分）。
/// 「按度数收电损」和「按电费收服务费」在行业里都存在，硬凑成同一个单位反而
/// 会让调用方偷偷做一次换算。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FeeBases {
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
    /// 某个基数当期的值。返回 `None` 表示基数名不认识。
    pub(crate) fn value(&self, rate_base: &str) -> Option<i64> {
        Some(match rate_base {
            RATE_BASE_FACTORY_USAGE => self.factory_usage_centi,
            RATE_BASE_FACTORY_FEE => self.factory_fee_cents,
            RATE_BASE_FACTORY_FEE_WITH_BASIC => {
                self.factory_fee_cents.saturating_add(self.basic_ele_fee_cents)
            }
            RATE_BASE_FACTORY_DORM_FEE => {
                self.factory_fee_cents.saturating_add(self.dormitory_fee_cents)
            }
            RATE_BASE_TOTAL_ELE_FEE => self.total_ele_fee_cents,
            _ => return None,
        })
    }
}

/// 一项约定费用当期该收多少分。
///
/// 固定金额直接返回；比例项按基数 × 比例算。基点是万分之一，所以除数是 10000。
///
/// 除法向零取整（Rust 整数除法的默认行为），也就是**永远不会多收**——四舍五入
/// 会在某些月份多收一分钱，而多收一分钱要解释起来比少收一分麻烦得多。
pub(crate) fn fee_amount_cents(fee: &RentalTenantFee, bases: &FeeBases) -> Result<i64, String> {
    match fee.charge_mode.as_str() {
        CHARGE_MODE_FIXED => Ok(fee.amount_cents.unwrap_or(0)),
        CHARGE_MODE_RATE => {
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

/// 基本电费 = 计费容量 × 单价。两项任一为空即不收。
///
/// 容量乘一百、单价乘一亿，乘出来是分乘以一亿乘以一百，所以要除掉 `1e8 × 100`。
pub(crate) fn basic_ele_fee_cents(
    capacity_centi_kw: Option<i64>,
    price_scaled: Option<i64>,
) -> i64 {
    let (Some(capacity), Some(price)) = (capacity_centi_kw, price_scaled) else {
        return 0;
    };
    // 单价是元/kW，结果要分，所以再乘一百；正好和容量的百倍抵消。
    ((capacity as i128 * price as i128) / 100_000_000) as i64
}

/// 一行电费明细在计费基数里的归属。
///
/// 由电表装在哪一层算出，不由用户逐条选——安装位置是台账里的事实，不是每个月
/// 要重新回答一遍的问题。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MeterScene {
    Factory,
    Dormitory,
    /// 公共区域的表，或者手工补录、没有台账可指的行。
    Public,
}

/// 把当期电费明细汇总成各个计费基数。
///
/// 入参是 `(场景, 计费用量, 金额)` 三元组而不是账单行，好让这一步不碰数据库：
/// 「哪一段该算进哪个基数」是唯一会算错的地方，必须能单独测。
pub(crate) fn collect_bases(
    rows: impl Iterator<Item=(MeterScene, i64, i64)>,
    basic_ele_fee_cents: i64,
) -> FeeBases {
    let mut bases = FeeBases {
        basic_ele_fee_cents,
        ..Default::default()
    };
    for (scene, usage_centi, amount_cents) in rows {
        bases.total_ele_fee_cents = bases.total_ele_fee_cents.saturating_add(amount_cents);
        match scene {
            MeterScene::Factory => {
                bases.factory_usage_centi = bases.factory_usage_centi.saturating_add(usage_centi);
                bases.factory_fee_cents = bases.factory_fee_cents.saturating_add(amount_cents);
            }
            MeterScene::Dormitory => {
                bases.dormitory_fee_cents =
                    bases.dormitory_fee_cents.saturating_add(amount_cents);
            }
            // 公共区域进总数，但不属于厂房也不属于宿舍——硬塞进厂房会让按
            // 「厂房电费合计」收的电损凭空变大。
            MeterScene::Public => {}
        }
    }
    bases
}

/// 账单上这三项周期性费用提交了多少分。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SubmittedFees {
    pub loss_cents: i64,
    pub service_cents: i64,
    pub garbage_cents: i64,
}

impl SubmittedFees {
    fn of(&self, fee_kind: &str) -> Option<i64> {
        Some(match fee_kind {
            FEE_KIND_LOSS => self.loss_cents,
            FEE_KIND_SERVICE => self.service_cents,
            FEE_KIND_GARBAGE => self.garbage_cents,
            _ => return None,
        })
    }
}

/// 按合同约定重算，与提交上来的金额逐项对账。
///
/// # 为什么服务端要再算一遍
///
/// 金额原来完全由客户端算好提交，服务端只存不验——也就是任何人都能提交任意
/// 金额。电损费、服务费、基本电费都是钱，这一层不能只靠界面。
///
/// # 只校验合同约定了的项
///
/// 合同没约定某一项，说明这一项没有自动规则，账单上手工填多少都是业务决定，
/// 不该拦。约定了的项则必须严格相等——「签一次、每月自动算」这件事如果允许
/// 随手覆盖，约定本身就没有意义了。要改金额就去改合同。
#[pure_function::pure]
pub(crate) fn check_agreed_fees(
    fees: &[RentalTenantFee],
    bases: &FeeBases,
    submitted: SubmittedFees,
) -> Result<(), String> {
    for fee in fees {
        let Some(actual) = submitted.of(&fee.fee_kind) else {
            continue;
        };
        let expected = fee_amount_cents(fee, bases)?;
        if actual != expected {
            return Err(format!(
                "{}与合同约定对不上：按合同算出 {} 元，账单上填的是 {} 元。改金额请先改合同约定。",
                fee_kind_label(&fee.fee_kind),
                format_cents(expected),
                format_cents(actual),
            ));
        }
    }
    Ok(())
}

/// 基本电费必须和它自己的算式对得上。
///
/// 校验的是**账单里存的那份快照**，不是合同的当前值：账单是对外的凭据，自带
/// 算式；合同以后改了容量或单价，已开出去的账单不该因此变得「非法」而编辑不了。
/// 快照缺一半就算不出金额，那样的条款签了也收不上来，直接拦掉。
#[pure_function::pure]
pub(crate) fn check_basic_ele_fee(
    capacity_centi_kw: Option<i64>,
    price_scaled: Option<i64>,
    submitted_cents: i64,
) -> Result<(), String> {
    if capacity_centi_kw.is_some() != price_scaled.is_some() {
        return Err("基本电费的计费容量和单价要么都填、要么都不填".into());
    }
    let expected = basic_ele_fee_cents(capacity_centi_kw, price_scaled);
    if submitted_cents != expected {
        return Err(format!(
            "基本电费与算式对不上：{} 元，按容量×单价应为 {} 元。",
            format_cents(submitted_cents),
            format_cents(expected),
        ));
    }
    Ok(())
}

/// 分转成「元」的文本，只用于错误消息。
fn format_cents(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, (cents % 100).abs())
}

/// 整体替换某份合同的约定费用。传空列表即清空。
#[spacetimedb::reducer]
pub fn set_rental_tenant_fees(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    fees: Vec<RentalTenantFeeInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let tenant = require_rental_tenant(ctx, rental_tenant_id)?;
    replace_rental_tenant_fees(ctx, rental_tenant_id, &tenant.customer_id, fees)
}

/// 校验并整体重写某份合同的约定费用。整批校验通过之后才落库。
pub(super) fn replace_rental_tenant_fees(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: &str,
    fees: Vec<RentalTenantFeeInput>,
) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut rows = Vec::with_capacity(fees.len());
    for fee in fees {
        let row = validated_fee(ctx, rental_tenant_id, customer_id, fee)?;
        if !seen.insert(row.fee_kind.clone()) {
            return Err(format!("同一种费用「{}」重复约定", fee_kind_label(&row.fee_kind)));
        }
        rows.push(row);
    }

    delete_rental_tenant_fees(ctx, rental_tenant_id);
    for row in rows {
        ctx.db.rental_tenant_fee().insert(row);
    }
    Ok(())
}

pub(super) fn delete_rental_tenant_fees(ctx: &ReducerContext, rental_tenant_id: u64) {
    let ids = ctx
        .db
        .rental_tenant_fee()
        .rental_tenant_fee_by_tenant()
        .filter(rental_tenant_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in ids {
        ctx.db.rental_tenant_fee().id().delete(id);
    }
}

/// 费用种类给用户看的名字。
pub(crate) fn fee_kind_label(fee_kind: &str) -> &'static str {
    match fee_kind {
        FEE_KIND_LOSS => "电损费",
        FEE_KIND_SERVICE => "服务费",
        FEE_KIND_GARBAGE => "垃圾费",
        _ => "未知费用",
    }
}

fn validated_fee(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: &str,
    input: RentalTenantFeeInput,
) -> Result<RentalTenantFee, String> {
    let fee_kind = required_text(input.fee_kind, "费用种类不能为空")?;
    if !FEE_KINDS.contains(&fee_kind.as_str()) {
        return Err(format!("不认识的费用种类「{fee_kind}」"));
    }
    let charge_mode = required_text(input.charge_mode, "计费方式不能为空")?;
    if !CHARGE_MODES.contains(&charge_mode.as_str()) {
        return Err(format!("不认识的计费方式「{charge_mode}」"));
    }

    // 两种计费方式各自只保留自己那一组字段：留着另一组的残值，界面切换回去时
    // 会显示上一次的数字，用户以为改过了其实没生效。
    let (amount_cents, rate_basis_points, rate_base) = match charge_mode.as_str() {
        CHARGE_MODE_FIXED => {
            let amount = input.amount_cents.ok_or("按固定金额计费时必须填写金额")?;
            if amount < 0 {
                return Err("费用金额不能为负数".into());
            }
            (Some(amount), None, None)
        }
        _ => {
            let rate = input.rate_basis_points.ok_or("按比例计费时必须填写比例")?;
            if !(0..=1_000_000).contains(&rate) {
                return Err("费用比例必须在 0% 到 100% 之间".into());
            }
            let base = required_text(
                input.rate_base.unwrap_or_default(),
                "按比例计费时必须选择计费基数",
            )?;
            if !RATE_BASES.contains(&base.as_str()) {
                return Err(format!("不认识的计费基数「{base}」"));
            }
            (None, Some(rate), Some(base))
        }
    };

    let remark = normalize_optional_text(input.remark);
    if let Some(remark) = &remark {
        validate_max_length(remark, 200, "费用备注不能超过 200 个字符")?;
    }

    Ok(RentalTenantFee {
        id: 0,
        customer_id: customer_id.to_string(),
        rental_tenant_id,
        fee_kind,
        charge_mode,
        amount_cents,
        rate_basis_points,
        rate_base,
        remark,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agreed(charge_mode: &str, amount: Option<i64>, rate: Option<i64>, base: Option<&str>) -> RentalTenantFee {
        RentalTenantFee {
            id: 1,
            customer_id: "public".into(),
            rental_tenant_id: 1,
            fee_kind: FEE_KIND_LOSS.into(),
            charge_mode: charge_mode.into(),
            amount_cents: amount,
            rate_basis_points: rate,
            rate_base: base.map(str::to_string),
            remark: None,
            created_at: spacetimedb::Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn bases() -> FeeBases {
        FeeBases {
            factory_usage_centi: 1_000_00,  // 1000 度
            factory_fee_cents: 100_000,     // 1000 元
            dormitory_fee_cents: 30_000,    // 300 元
            total_ele_fee_cents: 150_000,   // 1500 元（含公共区域 200 元）
            basic_ele_fee_cents: 20_000,    // 200 元
        }
    }

    #[test]
    fn 固定金额直接返回() {
        let fee = agreed(CHARGE_MODE_FIXED, Some(80_000), None, None);
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(80_000));
    }

    #[test]
    fn 比例项按各自基数计算() {
        // 5% × 厂房电费 1000 元 = 50 元
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_FACTORY_FEE));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(5_000));

        // 5% × （厂房电费 1000 + 基本电费 200）= 60 元
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_FACTORY_FEE_WITH_BASIC));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(6_000));

        // 5% × （厂房 1000 + 宿舍 300）= 65 元
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_FACTORY_DORM_FEE));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(6_500));

        // 5% × 全部电费 1500 元 = 75 元。比「厂房宿舍合计」多出来的是公共区域。
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_TOTAL_ELE_FEE));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(7_500));
    }

    #[test]
    fn 按度数收的电损基数单位是度不是钱() {
        // 5% × 1000 度 = 50 度。返回值此时是「度乘以一百」，调用方要再乘单价。
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_FACTORY_USAGE));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(5_000));
    }

    #[test]
    fn 除不尽时向零取整而不是四舍五入() {
        // 3.33% × 1000 元 = 33.3 元，取 33.30 元而不是 33.31。
        let fee = agreed(CHARGE_MODE_RATE, None, Some(333), Some(RATE_BASE_FACTORY_FEE));
        assert_eq!(fee_amount_cents(&fee, &bases()), Ok(3_330));

        // 1 基点 × 1 分 = 0.0001 分，向零取整成 0——宁可不收，不能凭空多收。
        let tiny = FeeBases { factory_fee_cents: 1, ..Default::default() };
        let fee = agreed(CHARGE_MODE_RATE, None, Some(1), Some(RATE_BASE_FACTORY_FEE));
        assert_eq!(fee_amount_cents(&fee, &tiny), Ok(0));
    }

    #[test]
    fn 基数缺失或不认识时报错而不是当成零() {
        // 当成零就是静默少收一笔钱，没人会发现。
        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), None);
        assert!(fee_amount_cents(&fee, &bases()).is_err());

        let fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some("厂房电费"));
        assert!(fee_amount_cents(&fee, &bases()).unwrap_err().contains("计费基数"));
    }

    #[test]
    fn 基本电费按容量乘单价() {
        // 500 kW × 23 元/kW = 11500 元
        assert_eq!(basic_ele_fee_cents(Some(500_00), Some(23 * 100_000_000)), 1_150_000);
        // 带小数的容量：123.45 kW × 23 元/kW = 2839.35 元
        assert_eq!(basic_ele_fee_cents(Some(123_45), Some(23 * 100_000_000)), 283_935);
    }

    #[test]
    fn 容量或单价缺一即不收基本电费() {
        assert_eq!(basic_ele_fee_cents(None, Some(23 * 100_000_000)), 0);
        assert_eq!(basic_ele_fee_cents(Some(500_00), None), 0);
        assert_eq!(basic_ele_fee_cents(None, None), 0);
    }

    #[test]
    fn 基数按场景分桶且公共区域只进总数() {
        let rows = [
            (MeterScene::Factory, 100_000, 100_000),   // 1000 度 / 1000 元
            (MeterScene::Dormitory, 30_000, 30_000),   // 300 元
            (MeterScene::Public, 20_000, 20_000),      // 200 元
        ];
        let bases = collect_bases(rows.into_iter(), 20_000);
        assert_eq!(bases.factory_usage_centi, 100_000);
        assert_eq!(bases.factory_fee_cents, 100_000);
        assert_eq!(bases.dormitory_fee_cents, 30_000);
        // 公共区域进总数，但不进厂房也不进宿舍。
        assert_eq!(bases.total_ele_fee_cents, 150_000);
        assert_eq!(bases.basic_ele_fee_cents, 20_000);
    }

    fn loss_five_percent_of_factory() -> RentalTenantFee {
        let mut fee = agreed(CHARGE_MODE_RATE, None, Some(500), Some(RATE_BASE_FACTORY_FEE));
        fee.fee_kind = FEE_KIND_LOSS.into();
        fee
    }

    #[test]
    fn 约定项的金额必须等于按合同算出来的值() {
        let fees = vec![loss_five_percent_of_factory()];
        // 5% × 厂房电费 1000 元 = 50 元
        let ok = check_agreed_fees(
            &fees,
            &bases(),
            SubmittedFees { loss_cents: 5_000, ..Default::default() },
        );
        assert_eq!(ok, Ok(()));

        let message = check_agreed_fees(
            &fees,
            &bases(),
            SubmittedFees { loss_cents: 9_900, ..Default::default() },
        )
            .expect_err("多收了 49 元必须拦下");
        // 报错要同时给出两个数，否则用户不知道该改哪一边。
        assert!(message.contains("电损费"), "{message}");
        assert!(message.contains("50.00"), "{message}");
        assert!(message.contains("99.00"), "{message}");
    }

    #[test]
    fn 合同没约定的项不受管() {
        // 没有任何约定时，账单上手工填的服务费、垃圾费都是业务决定，不该拦。
        let ok = check_agreed_fees(
            &[],
            &bases(),
            SubmittedFees { service_cents: 80_000, garbage_cents: 12_000, loss_cents: 3_000 },
        );
        assert_eq!(ok, Ok(()));

        // 只约定了电损，服务费仍然自由。
        let ok = check_agreed_fees(
            &[loss_five_percent_of_factory()],
            &bases(),
            SubmittedFees { loss_cents: 5_000, service_cents: 80_000, garbage_cents: 0 },
        );
        assert_eq!(ok, Ok(()));
    }

    #[test]
    fn 基本电费必须和自己的快照算式对得上() {
        // 500 kW × 23 元/kW = 11500 元
        assert_eq!(
            check_basic_ele_fee(Some(50_000), Some(23 * 100_000_000), 1_150_000),
            Ok(())
        );
        let message = check_basic_ele_fee(Some(50_000), Some(23 * 100_000_000), 999_999)
            .expect_err("金额和算式对不上必须拦下");
        assert!(message.contains("11500.00"), "{message}");

        // 没有条款时金额必须是 0，否则就是凭空多收一笔。
        assert!(check_basic_ele_fee(None, None, 100).is_err());
        assert_eq!(check_basic_ele_fee(None, None, 0), Ok(()));

        // 只填一半算不出金额，签了也收不上来。
        assert!(check_basic_ele_fee(Some(50_000), None, 0).is_err());
    }
}
