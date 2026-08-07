//! 合同用表关系与合同约定水电单价的维护。
//!
//! 语义与楼层归属一致：整体替换。一次调用给出这份合同用到的全部水电表
//! 及各自约定单价，服务端删旧建新，调用方不必自己算差集。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{require_rental_tenant, AdminContext},
        rental::assets::require_utility_meter,
        validation::normalize_optional_text,
    },
    tables::*,
};

/// 合同对某一块表约定的结算价。
///
/// 单一价和分时四段二选一：普通表按 `unit_price_scaled` 结算，分时表可以
/// 按四段结算、也可以谈一口价。全部为空是不合法的——那份合同到期开账单时
/// 拿不到任何计价依据。
#[derive(SpacetimeType)]
pub struct RentalTenantMeterInput {
    pub meter_id: u64,
    pub unit_price_scaled: Option<i64>,
    pub price_tip_scaled: Option<i64>,
    pub price_peak_scaled: Option<i64>,
    pub price_flat_scaled: Option<i64>,
    pub price_valley_scaled: Option<i64>,
    pub remark: Option<String>,
}

/// 整体替换某份合同的用表关系。传空列表即解除全部关联。
#[spacetimedb::reducer]
pub fn set_rental_tenant_meters(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    meters: Vec<RentalTenantMeterInput>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let tenant = require_rental_tenant(ctx, rental_tenant_id)?;
    replace_rental_tenant_meters(ctx, rental_tenant_id, &tenant.customer_id, meters)
}

/// 校验并整体重写某份合同的用表关系。整批校验通过之后才落库。
pub(super) fn replace_rental_tenant_meters(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: &str,
    meters: Vec<RentalTenantMeterInput>,
) -> Result<(), String> {
    let tenancy = ctx
        .db
        .rental_tenant()
        .rental_tenant_id()
        .find(rental_tenant_id)
        .map(|row| (row.contract_start, row.contract_end))
        .unwrap_or((None, None));

    let mut seen = std::collections::BTreeSet::new();
    for entry in &meters {
        let meter = require_utility_meter(ctx, entry.meter_id)?;
        check_pricing(entry, meter.is_time_of_use, &meter.meter_code)?;
        if !seen.insert(entry.meter_id) {
            return Err(format!("水电表「{}」重复出现", meter.meter_code));
        }
        require_no_concurrent_holder(ctx, rental_tenant_id, &meter, tenancy)?;
    }

    delete_rental_tenant_meter_links(ctx, rental_tenant_id);
    for entry in meters {
        ctx.db.rental_tenant_meter().insert(RentalTenantMeter {
            id: 0,
            customer_id: customer_id.to_string(),
            rental_tenant_id,
            meter_id: entry.meter_id,
            unit_price_scaled: entry.unit_price_scaled,
            price_tip_scaled: entry.price_tip_scaled,
            price_peak_scaled: entry.price_peak_scaled,
            price_flat_scaled: entry.price_flat_scaled,
            price_valley_scaled: entry.price_valley_scaled,
            remark: normalize_optional_text(entry.remark),
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

/// 合同被删除时一并清掉它的用表关联。
pub(super) fn delete_rental_tenant_meter_links(ctx: &ReducerContext, rental_tenant_id: u64) {
    for link in ctx
        .db
        .rental_tenant_meter()
        .rental_tenant_meter_by_tenant()
        .filter(rental_tenant_id)
        .collect::<Vec<_>>()
    {
        ctx.db.rental_tenant_meter().id().delete(link.id);
    }
}

/// 同一块表不能在同一段时间里挂给两份合同。
///
/// 只在能够证明重叠时才拦截，见 [`tenancy_overlaps`]。
fn require_no_concurrent_holder(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    meter: &UtilityMeter,
    tenancy: (Option<Timestamp>, Option<Timestamp>),
) -> Result<(), String> {
    for link in ctx
        .db
        .rental_tenant_meter()
        .rental_tenant_meter_by_meter()
        .filter(meter.meter_id)
        .filter(|link| link.rental_tenant_id != rental_tenant_id)
    {
        let Some(other) = ctx
            .db
            .rental_tenant()
            .rental_tenant_id()
            .find(link.rental_tenant_id)
            .filter(|row| !row.is_deleted)
        else {
            continue;
        };
        if tenancy_overlaps(tenancy, (other.contract_start, other.contract_end)) {
            return Err(format!(
                "水电表「{}」在同一租期内已挂给合同「{}」",
                meter.meter_code, other.tenant_name
            ));
        }
    }
    Ok(())
}

/// 两份合同的租期是否重叠。
///
/// 判定方向是「只在能证明重叠时才拦」：两份合同的起止时间都齐全、且区间
/// 确实相交，才算冲突；任何一方缺时间就放行。
///
/// 反过来把缺失值当成无穷大在纸面上更"安全"，但生产库里的历史合同大量
/// 没有填起止时间，那样判定会让它们两两冲突、任何一块表都挂不上第二份
/// 合同——连「去年那户搬走了、今年换一户」这种完全正常的续租都做不了。
/// 时间不全时的重复用表交给页面提示，不在这里硬拦。
fn tenancy_overlaps(
    a: (Option<Timestamp>, Option<Timestamp>),
    b: (Option<Timestamp>, Option<Timestamp>),
) -> bool {
    let (Some(a_start), Some(a_end)) = a else {
        return false;
    };
    let (Some(b_start), Some(b_end)) = b else {
        return false;
    };
    a_start <= b_end && b_start <= a_end
}

/// 单价填写方式的校验。
///
/// 抽成纯函数是为了能覆盖「只填了一半分时电价」这类边界——四段里漏填一段
/// 的合同到期开账单时会算出一个漏掉整段用电量的金额，比直接报错难查得多。
#[pure_function::pure]
fn check_pricing(
    input: &RentalTenantMeterInput,
    is_time_of_use: bool,
    meter_code: &str,
) -> Result<(), String> {
    let tiers = [
        input.price_tip_scaled,
        input.price_peak_scaled,
        input.price_flat_scaled,
        input.price_valley_scaled,
    ];
    if input.unit_price_scaled.is_some_and(|value| value < 0)
        || tiers.iter().any(|tier| tier.is_some_and(|value| value < 0))
    {
        return Err(format!("水电表「{meter_code}」的单价不能为负数"));
    }
    let filled = tiers.iter().filter(|tier| tier.is_some()).count();
    if filled != 0 && filled != 4 {
        return Err(format!(
            "水电表「{meter_code}」的分时电价必须尖峰平谷四段全部填写"
        ));
    }
    if filled == 4 && !is_time_of_use {
        return Err(format!(
            "水电表「{meter_code}」不是分时表，不能约定分时电价"
        ));
    }
    match (input.unit_price_scaled.is_some(), filled == 4) {
        (false, false) => Err(format!("请为水电表「{meter_code}」填写结算单价")),
        (true, true) => Err(format!(
            "水电表「{meter_code}」的单一单价和分时电价只能二选一"
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(unit: Option<i64>, tiers: Option<[i64; 4]>) -> RentalTenantMeterInput {
        RentalTenantMeterInput {
            meter_id: 1,
            unit_price_scaled: unit,
            price_tip_scaled: tiers.map(|value| value[0]),
            price_peak_scaled: tiers.map(|value| value[1]),
            price_flat_scaled: tiers.map(|value| value[2]),
            price_valley_scaled: tiers.map(|value| value[3]),
            remark: None,
        }
    }

    #[test]
    fn 单一单价与分时电价二选一() {
        assert!(check_pricing(&input(Some(105_000_000), None), false, "A1").is_ok());
        assert!(check_pricing(
            &input(None, Some([150, 120, 90, 50])),
            true,
            "A1"
        )
        .is_ok());
        // 两种都填等于没说按哪种结算。
        assert!(check_pricing(
            &input(Some(105_000_000), Some([150, 120, 90, 50])),
            true,
            "A1"
        )
        .is_err());
        // 两种都不填，开账单时没有计价依据。
        assert!(check_pricing(&input(None, None), true, "A1").is_err());
    }

    #[test]
    fn 分时电价填一半被拦截() {
        let mut half = input(None, Some([150, 120, 90, 50]));
        half.price_valley_scaled = None;
        let error = check_pricing(&half, true, "A1").unwrap_err();
        assert!(error.contains("四段全部填写"), "{error}");
    }

    #[test]
    fn 不分时的表不能约定分时电价() {
        // 表本身不分时计量，四段价格永远拿不到对应的分段用电量。
        assert!(check_pricing(&input(None, Some([150, 120, 90, 50])), false, "水表1").is_err());
    }

    #[test]
    fn 负单价被拦截() {
        assert!(check_pricing(&input(Some(-1), None), false, "A1").is_err());
        assert!(check_pricing(&input(None, Some([150, 120, 90, -1])), true, "A1").is_err());
    }

    fn at(micros: i64) -> Option<Timestamp> {
        Some(Timestamp::from_micros_since_unix_epoch(micros))
    }

    #[test]
    fn 前后相继的租期不算重叠() {
        // 去年那户搬走、今年换一户，同一块表理应挂得上去。
        assert!(!tenancy_overlaps((at(100), at(200)), (at(300), at(400))));
        assert!(!tenancy_overlaps((at(300), at(400)), (at(100), at(200))));
    }

    #[test]
    fn 交叠的租期被判定为重叠() {
        assert!(tenancy_overlaps((at(100), at(300)), (at(200), at(400))));
        // 首尾正好相接的同一天也算占用冲突。
        assert!(tenancy_overlaps((at(100), at(200)), (at(200), at(400))));
    }

    #[test]
    fn 任一方缺时间就放行() {
        // 历史合同大量没填起止时间，按无穷大判定会让它们两两冲突。
        assert!(!tenancy_overlaps((None, None), (at(100), at(200))));
        assert!(!tenancy_overlaps((at(100), None), (at(200), at(400))));
        assert!(!tenancy_overlaps((None, None), (None, None)));
    }
}
