//! 水电表台账的增删改。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_dormitory, require_factory, require_park,
            require_park_access, require_rental_manager,
        },
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

/// 水电表可修改字段。
#[derive(SpacetimeType)]
pub struct UtilityMeterInput {
    pub park_id: u64,
    pub meter_code: String,
    pub is_electric: bool,
    /// 装在哪个厂房楼层，0 表示不在厂房。
    pub factory_floor_id: u64,
    /// 装在哪个宿舍楼层，0 表示不在宿舍。两者同时为 0 即园区公共表。
    pub dormitory_floor_id: u64,
    pub multiplier_centi: i64,
    pub is_time_of_use: bool,
    pub external_device_id: Option<String>,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_utility_meter(ctx: &ReducerContext, input: UtilityMeterInput) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_meter(ctx, 0, customer_id, input)?;
    ctx.db.utility_meter().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_utility_meter(
    ctx: &ReducerContext,
    meter_id: u64,
    input: UtilityMeterInput,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let existing = require_utility_meter(ctx, meter_id)?;
    // 分时能力被关掉时，挂在这块表上的分时报价就再也算不出账——先解除
    // 合同关联，避免留下一份永远无法结算的合同条款。
    if existing.is_time_of_use && !input.is_time_of_use {
        let tiered = ctx
            .db
            .rental_tenant_meter()
            .rental_tenant_meter_by_meter()
            .filter(meter_id)
            .filter(|link| link.price_flat_scaled.is_some())
            .count();
        if tiered > 0 {
            return Err(format!(
                "已有 {tiered} 份合同对这块表约定了分时电价，取消分时前请先改这些合同的报价"
            ));
        }
    }
    let mut row = validated_meter(ctx, meter_id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.utility_meter().meter_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_utility_meter(ctx: &ReducerContext, meter_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let mut meter = require_utility_meter(ctx, meter_id)?;
    require_park_access(ctx, meter.park_id)?;
    // 表是账单的计价依据，被合同引用时直接删掉会让那份合同失去水电条款。
    let used = ctx
        .db
        .rental_tenant_meter()
        .rental_tenant_meter_by_meter()
        .filter(meter_id)
        .filter(|link| {
            ctx.db
                .rental_tenant()
                .rental_tenant_id()
                .find(link.rental_tenant_id)
                .is_some_and(|tenant| !tenant.is_deleted)
        })
        .count();
    if used > 0 {
        return Err(format!(
            "这块表还被 {used} 份合同引用，请先解除合同关联再删除"
        ));
    }
    meter.is_deleted = true;
    meter.updated_at = Some(ctx.timestamp);
    ctx.db.utility_meter().meter_id().update(meter);
    Ok(())
}

pub(crate) fn require_utility_meter(
    ctx: &ReducerContext,
    meter_id: u64,
) -> Result<UtilityMeter, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .utility_meter()
        .meter_id()
        .find(meter_id)
        .filter(|meter| meter.customer_id == customer_id && !meter.is_deleted)
        .ok_or("水电表不存在".into())
}

fn validated_meter(
    ctx: &ReducerContext,
    meter_id: u64,
    customer_id: String,
    input: UtilityMeterInput,
) -> Result<UtilityMeter, String> {
    let meter_code = required_text(input.meter_code, "表号不能为空")?;
    require_park(ctx, input.park_id)?;
    require_park_access(ctx, input.park_id)?;
    check_single_location(input.factory_floor_id, input.dormitory_floor_id)?;
    if input.multiplier_centi <= 0 {
        return Err("倍率必须大于零".into());
    }
    // 安装位置必须落在这块表自己的园区里，否则台账会出现「A 园区的表装在
    // B 园区楼上」这种自相矛盾的记录。
    if input.factory_floor_id != 0 {
        let floor = ctx
            .db
            .factory_floor()
            .floor_id()
            .find(input.factory_floor_id)
            .filter(|floor| floor.customer_id == customer_id && !floor.is_deleted)
            .ok_or("厂房楼层不存在")?;
        let factory = require_factory(ctx, floor.factory_id)?;
        if factory.park_id != input.park_id {
            return Err("厂房楼层不属于所选园区".into());
        }
    }
    if input.dormitory_floor_id != 0 {
        let floor = ctx
            .db
            .dormitory_floor()
            .dormitory_floor_id()
            .find(input.dormitory_floor_id)
            .filter(|floor| floor.customer_id == customer_id && !floor.is_deleted)
            .ok_or("宿舍楼层不存在")?;
        let dormitory = require_dormitory(ctx, floor.dormitory_id)?;
        if dormitory.park_id != input.park_id {
            return Err("宿舍楼层不属于所选园区".into());
        }
    }
    // 表号在园区内唯一：抄表和对账都靠表号认表，重号等于两块表混成一块。
    let duplicated = ctx
        .db
        .utility_meter()
        .utility_meter_by_park()
        .filter(input.park_id)
        .any(|row| {
            !row.is_deleted && row.meter_id != meter_id && row.meter_code.trim() == meter_code
        });
    if duplicated {
        return Err(format!("园区内已存在表号「{meter_code}」"));
    }
    // 一台智能水电表设备只能绑一块表，而且是全租户唯一而不是园区内唯一——
    // 设备号是供应商侧的身份，同一台设备绑两块表会让同一份读数被两块表各算
    // 一次，账单直接出双份。范围要覆盖整个租户，否则跨园区重绑照样漏过。
    let external_device_id = normalize_optional_text(input.external_device_id);
    if let Some(device_id) = &external_device_id {
        let bound = ctx
            .db
            .utility_meter()
            .utility_meter_by_customer()
            .filter(customer_id.as_str())
            .find(|row| {
                !row.is_deleted
                    && row.meter_id != meter_id
                    && row.external_device_id.as_deref() == Some(device_id.as_str())
            });
        if let Some(bound) = bound {
            return Err(format!(
                "智能水电表设备 {device_id} 已经绑定给表「{}」，请先解绑",
                bound.meter_code
            ));
        }
    }
    Ok(UtilityMeter {
        meter_id,
        customer_id,
        park_id: input.park_id,
        meter_code,
        is_electric: input.is_electric,
        factory_floor_id: input.factory_floor_id,
        dormitory_floor_id: input.dormitory_floor_id,
        multiplier_centi: input.multiplier_centi,
        // 只有电表谈得上分时；水表强制关掉，免得台账里出现分时水表。
        is_time_of_use: input.is_electric && input.is_time_of_use,
        external_device_id,
        remark: normalize_optional_text(input.remark),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

/// 一块表只能装在一个地方。
///
/// 两个位置同时为 0 是合法的——生产数据里 299 条「公共用电」就是装在园区
/// 公共区域、不属于任何一层的表。
#[pure_function::pure]
fn check_single_location(factory_floor_id: u64, dormitory_floor_id: u64) -> Result<(), String> {
    if factory_floor_id != 0 && dormitory_floor_id != 0 {
        return Err("一块表只能安装在厂房楼层或宿舍楼层其中之一".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 表只能装在一处() {
        assert!(check_single_location(1, 0).is_ok());
        assert!(check_single_location(0, 1).is_ok());
        assert!(check_single_location(1, 1).is_err());
    }

    #[test]
    fn 不挂楼层的公共表合法() {
        // 「公共用电」这类表装在园区公共区域，不属于任何一层。
        assert!(check_single_location(0, 0).is_ok());
    }
}
