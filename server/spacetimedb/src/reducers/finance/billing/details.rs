//! 水电账单明细事务。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_amount_bill},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 分时时段的合法取值。库里存英文标识，中文叫法留给界面。
pub(crate) const TOU_TIERS: [&str; 4] = ["tip", "peak", "flat", "valley"];

/// 水电表明细的公共输入字段。
#[derive(Clone, SpacetimeType)]
pub struct UtilityBillInput {
    /// 关联的台账表主键，`0` 表示手工补录、不指向任何表。
    pub meter_id: u64,
    /// 分时时段（`tip` / `peak` / `flat` / `valley`），非分时行传 `None`。
    pub tou_tier: Option<String>,
    pub meter_name: String,
    pub previous_reading_centi: i64,
    pub current_reading_centi: i64,
    pub monthly_usage_centi: i64,
    pub multiplier_centi: i64,
    pub total_usage_centi: i64,
    pub unit_price_scaled: i64,
    pub amount_cents: i64,
    pub remark: Option<String>,
    pub receipt_time: Option<Timestamp>,
}

#[spacetimedb::reducer]
pub fn add_ele_bill(
    ctx: &ReducerContext,
    bill_id: u64,
    input: UtilityBillInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_amount_bill(ctx, bill_id)?;
    let input = validated_utility(input, true)?;
    require_bound_meter(ctx, input.meter_id, true)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.ele_bill().insert(EleBill {
        ele_id: 0,
        customer_id,
        bill_id,
        meter_id: input.meter_id,
        tou_tier: input.tou_tier,
        meter_name: input.meter_name,
        previous_reading_centi: input.previous_reading_centi,
        current_reading_centi: input.current_reading_centi,
        monthly_usage_centi: input.monthly_usage_centi,
        multiplier_centi: input.multiplier_centi,
        total_usage_centi: input.total_usage_centi,
        unit_price_scaled: input.unit_price_scaled,
        amount_cents: input.amount_cents,
        remark: input.remark,
        receipt_time: input.receipt_time,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn add_water_bill(
    ctx: &ReducerContext,
    bill_id: u64,
    input: UtilityBillInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_amount_bill(ctx, bill_id)?;
    let input = validated_utility(input, false)?;
    require_bound_meter(ctx, input.meter_id, false)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db.water_bill().insert(WaterBill {
        water_id: 0,
        customer_id,
        bill_id,
        meter_id: input.meter_id,
        meter_name: input.meter_name,
        previous_reading_centi: input.previous_reading_centi,
        current_reading_centi: input.current_reading_centi,
        monthly_usage_centi: input.monthly_usage_centi,
        multiplier_centi: input.multiplier_centi,
        total_usage_centi: input.total_usage_centi,
        unit_price_scaled: input.unit_price_scaled,
        amount_cents: input.amount_cents,
        remark: input.remark,
        receipt_time: input.receipt_time,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_ele_bill(ctx: &ReducerContext, ele_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = ctx
        .db
        .ele_bill()
        .ele_id()
        .find(ele_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("电费明细不存在")?;
    ctx.db.ele_bill().ele_id().delete(row.ele_id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_water_bill(ctx: &ReducerContext, water_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = ctx
        .db
        .water_bill()
        .water_id()
        .find(water_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("水费明细不存在")?;
    ctx.db.water_bill().water_id().delete(row.water_id);
    Ok(())
}

/// 校验明细指向的表存在、属于本租户、且水电类型对得上。
///
/// `0` 表示不指向任何表，直接放行——历史数据和手工补录的行没有表可指。
/// 类型必须一致：把水表填进电费明细，账单会按电价算水的用量。
pub(super) fn require_bound_meter(
    ctx: &ReducerContext,
    meter_id: u64,
    is_electric: bool,
) -> Result<(), String> {
    if meter_id == 0 {
        return Ok(());
    }
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let meter = ctx
        .db
        .utility_meter()
        .meter_id()
        .find(meter_id)
        .filter(|meter| meter.customer_id == customer_id && !meter.is_deleted)
        .ok_or("明细关联的水电表不存在")?;
    if meter.is_electric != is_electric {
        return Err(format!(
            "表「{}」是{}，不能记在{}明细里",
            meter.meter_code,
            if meter.is_electric { "电表" } else { "水表" },
            if is_electric { "电费" } else { "水费" }
        ));
    }
    Ok(())
}

/// 校验分时标记本身是否合法，以及它和表计类型是否自相符。
///
/// 水表没有分时，带着分时标记的水费明细一定是客户端串了参数——放过去的后果
/// 是同一块水表一个账期出四行，账单金额直接翻四倍。
#[pure_function::pure]
pub(super) fn check_tou_tier(tou_tier: Option<&str>, is_electric: bool) -> Result<(), String> {
    let Some(tier) = tou_tier else {
        return Ok(());
    };
    if !is_electric {
        return Err("水费明细不支持分时时段".into());
    }
    if !TOU_TIERS.contains(&tier) {
        return Err(format!("分时时段「{tier}」不是尖峰平谷中的一个"));
    }
    Ok(())
}

pub(super) fn validated_utility(
    mut input: UtilityBillInput,
    is_electric: bool,
) -> Result<UtilityBillInput, String> {
    input.meter_name = required_text(input.meter_name, "表计名称不能为空")?;
    input.tou_tier = normalize_optional_text(input.tou_tier);
    check_tou_tier(input.tou_tier.as_deref(), is_electric)?;
    let values = [
        input.previous_reading_centi,
        input.current_reading_centi,
        input.monthly_usage_centi,
        input.multiplier_centi,
        input.total_usage_centi,
        input.unit_price_scaled,
        input.amount_cents,
    ];
    if values.into_iter().any(|value| value < 0) {
        return Err("水电明细数值不能为负数".into());
    }
    if input.current_reading_centi < input.previous_reading_centi {
        return Err("本月抄表数不能小于上月抄表数".into());
    }
    // 原收款通知单公式由 Module 重算，客户端提交的派生值不作为可信来源。
    input.monthly_usage_centi = input.current_reading_centi - input.previous_reading_centi;
    input.total_usage_centi =
        ((input.monthly_usage_centi as i128 * input.multiplier_centi as i128) / 100)
            .try_into()
            .map_err(|_| "水电实际用量数值过大")?;
    input.amount_cents = ((input.total_usage_centi as i128 * input.unit_price_scaled as i128
        + 50_000_000)
        / 100_000_000)
        .try_into()
        .map_err(|_| "水电金额数值过大")?;
    input.remark = normalize_optional_text(input.remark);
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 水电明细派生值由服务端按原表格公式重算() {
        let input = UtilityBillInput {
            meter_id: 0,
            tou_tier: None,
            meter_name: "8303".into(),
            previous_reading_centi: 10_000,
            current_reading_centi: 12_550,
            monthly_usage_centi: 1,
            multiplier_centi: 200,
            total_usage_centi: 1,
            unit_price_scaled: 80_000_000,
            amount_cents: 1,
            remark: None,
            receipt_time: None,
        };
        let result = validated_utility(input, true).unwrap();
        assert_eq!(result.monthly_usage_centi, 2_550);
        assert_eq!(result.total_usage_centi, 5_100);
        assert_eq!(result.amount_cents, 4_080);
    }

    #[test]
    fn 分时时段只接受尖峰平谷四个标识() {
        for tier in TOU_TIERS {
            assert!(check_tou_tier(Some(tier), true).is_ok(), "{tier} 应当合法");
        }
        assert!(check_tou_tier(None, true).is_ok());
        assert!(check_tou_tier(Some("尖"), true).is_err(), "中文叫法不入库");
        assert!(check_tou_tier(Some("night"), true).is_err());
    }

    #[test]
    fn 水费明细不接受分时时段() {
        // 放过去的后果是同一块水表一个账期出四行，金额直接翻四倍。
        assert_eq!(
            check_tou_tier(Some("peak"), false),
            Err("水费明细不支持分时时段".into())
        );
        assert!(check_tou_tier(None, false).is_ok());
    }

    #[test]
    fn 分时标记会被规范化后再校验() {
        let input = UtilityBillInput {
            meter_id: 0,
            tou_tier: Some("  peak  ".into()),
            meter_name: "8303".into(),
            previous_reading_centi: 0,
            current_reading_centi: 100,
            monthly_usage_centi: 0,
            multiplier_centi: 100,
            total_usage_centi: 0,
            unit_price_scaled: 100_000_000,
            amount_cents: 0,
            remark: None,
            receipt_time: None,
        };
        let result = validated_utility(input, true).unwrap();
        assert_eq!(result.tou_tier.as_deref(), Some("peak"));
    }
}
