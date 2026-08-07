//! 租赁客户创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_rental_tenant},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

use super::tenant_floor::{
    delete_rental_tenant_dormitory_floor_links, delete_rental_tenant_floor_links,
    replace_rental_tenant_dormitory_floors, replace_rental_tenant_floors,
    RentalTenantDormitoryFloorInput, RentalTenantFloorInput,
};
use super::tenant_fee::{
    delete_rental_tenant_fees, replace_rental_tenant_fees, RentalTenantFeeInput,
};
use super::tenant_meter::{
    delete_rental_tenant_meter_links, replace_rental_tenant_meters, RentalTenantMeterInput,
};

/// 租赁客户及合同摘要的可修改字段。
#[derive(SpacetimeType)]
pub struct RentalTenantInput {
    pub tenant_name: String,
    pub phone_number: String,
    pub transaction_type: bool,
    pub status: Option<String>,
    pub contract_start: Option<Timestamp>,
    pub contract_end: Option<Timestamp>,
    pub rental_amount_cents: Option<i64>,
    pub increase_date: Option<Timestamp>,
    pub increase_rate_basis_points: Option<i64>,
    pub increase_data: Option<String>,
    pub penalty_rate_basis_points: Option<i64>,
    pub basic_ele_capacity_centi_kw: Option<i64>,
    pub basic_ele_price_scaled: Option<i64>,
    pub area_centi_square_metres: Option<i64>,
    pub remark: Option<String>,
    pub park_id: Option<u64>,
    pub send_message_at: Option<Timestamp>,
    /// 这份合同租用的楼层及各自占用面积。
    ///
    /// 跟着合同一起提交而不是单独调用一次 reducer：新建合同时前端拿不到
    /// 自增出来的 `rental_tenant_id`，分两步写就必须先建后查再补，中间
    /// 任何一步失败都会留下没有楼层归属的合同。
    pub floors: Vec<RentalTenantFloorInput>,
    /// 这份合同占用了哪几层宿舍、各占几间。
    ///
    /// 跟厂房楼层一起提交而不是单独调一次 reducer，理由同上：新建合同时
    /// 前端拿不到自增出来的主键，分两步写就会留下没有归属的合同。
    pub dormitory_floors: Vec<RentalTenantDormitoryFloorInput>,
    /// 这份合同用哪几块水电表，以及各自约定的结算单价。
    ///
    /// 水电定价是合同条款而不是账单的临时输入：同一份合同连续开十二个月
    /// 账单，单价谈定一次就该用十二次，而不是手敲十二遍。
    pub meters: Vec<RentalTenantMeterInput>,
    /// 这份合同约定的周期性费用（电损／服务／垃圾）。
    pub fees: Vec<RentalTenantFeeInput>,
}

#[spacetimedb::reducer]
pub fn create_rental_tenant(ctx: &ReducerContext, input: RentalTenantInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    upsert_tenant_with_links(ctx, 0, customer_id, input).map(|_| ())
}

#[spacetimedb::reducer]
pub fn update_rental_tenant(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    input: RentalTenantInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_rental_tenant(ctx, rental_tenant_id)?;
    upsert_tenant_with_links(ctx, rental_tenant_id, existing.customer_id, input).map(|_| ())
}

/// 写入合同主记录及其全部关联关系，返回合同主键。
///
/// 建行和写关联捆在一个函数里，是因为把它们分开曾经真的出过事：
/// `validated_tenant` 原本是模块内公开的，带图片的那两个 reducer
/// （合同表单实际调用的就是它们）直接拿它建行，`input.floors` 连同后来
/// 加的 `input.meters` 一起被无声地丢掉——用户在表单里勾好楼层、点保存、
/// 再打开又变回没勾，而且四个入口里只有两个有这个毛病，从调用点看不出来。
///
/// 现在 `validated_tenant` 私有，没有第二条路能建出一条不带关联的合同行。
/// 新增入口时不需要记得"还要写关联表"，因为压根没有别的函数可调。
///
/// `rental_tenant_id` 传 0 表示新建。
pub(super) fn upsert_tenant_with_links(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: String,
    mut input: RentalTenantInput,
) -> Result<u64, String> {
    let floors = std::mem::take(&mut input.floors);
    let dormitory_floors = std::mem::take(&mut input.dormitory_floors);
    let meters = std::mem::take(&mut input.meters);
    let fees = std::mem::take(&mut input.fees);
    let mut row = validated_tenant(ctx, rental_tenant_id, customer_id.clone(), input)?;
    let rental_tenant_id = if rental_tenant_id == 0 {
        // 自增主键要落库之后才拿得到，关联行只能在插入之后写。
        ctx.db.rental_tenant().insert(row).rental_tenant_id
    } else {
        let existing = require_rental_tenant(ctx, rental_tenant_id)?;
        row.created_at = existing.created_at;
        row.updated_at = Some(ctx.timestamp);
        ctx.db.rental_tenant().rental_tenant_id().update(row);
        rental_tenant_id
    };
    replace_rental_tenant_floors(ctx, rental_tenant_id, &customer_id, floors)?;
    replace_rental_tenant_dormitory_floors(ctx, rental_tenant_id, &customer_id, dormitory_floors)?;
    replace_rental_tenant_meters(ctx, rental_tenant_id, &customer_id, meters)?;
    replace_rental_tenant_fees(ctx, rental_tenant_id, &customer_id, fees)?;
    Ok(rental_tenant_id)
}

#[spacetimedb::reducer]
pub fn delete_rental_tenant(ctx: &ReducerContext, rental_tenant_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut tenant = require_rental_tenant(ctx, rental_tenant_id)?;
    let tenant_links = ctx
        .db
        .tenant_image()
        .tenant_image_by_tenant()
        .filter(rental_tenant_id)
        .collect::<Vec<_>>();
    for link in &tenant_links {
        ctx.db.tenant_image().id().delete(link.id);
    }
    for img_id in tenant_links.into_iter().map(|link| link.img_id) {
        crate::reducers::platform::media::image_reducer::delete_image_if_unreferenced(ctx, img_id);
    }
    // 工资已改为归属员工，不再随合同一起注销——删掉一份租赁合同不应该
    // 影响任何人的工资记录。
    delete_rental_tenant_floor_links(ctx, rental_tenant_id);
    delete_rental_tenant_dormitory_floor_links(ctx, rental_tenant_id);
    // 表本身是资产，留在台账里；解除的只是「这份合同用它」这层关系。
    delete_rental_tenant_meter_links(ctx, rental_tenant_id);
    delete_rental_tenant_fees(ctx, rental_tenant_id);
    tenant.is_deleted = true;
    tenant.updated_at = Some(ctx.timestamp);
    ctx.db.rental_tenant().rental_tenant_id().update(tenant);
    Ok(())
}

fn validated_tenant(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
    customer_id: String,
    input: RentalTenantInput,
) -> Result<RentalTenant, String> {
    let tenant_name = required_text(input.tenant_name, "租赁客户名称不能为空")?;
    let phone_number = required_text(input.phone_number, "联系电话不能为空")?;
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    if let (Some(start), Some(end)) = (input.contract_start, input.contract_end)
        && end < start
    {
        return Err("合同结束时间不能早于开始时间".into());
    }
    if input.rental_amount_cents.is_some_and(|value| value < 0)
        || input
        .increase_rate_basis_points
        .is_some_and(|value| value < 0)
        || input
        .penalty_rate_basis_points
        .is_some_and(|value| value < 0)
        || input
        .area_centi_square_metres
        .is_some_and(|value| value < 0)
        || input
        .basic_ele_capacity_centi_kw
        .is_some_and(|value| value < 0)
        || input.basic_ele_price_scaled.is_some_and(|value| value < 0)
    {
        return Err("租金、费率、面积和基本电费不能为负数".into());
    }
    Ok(RentalTenant {
        rental_tenant_id,
        customer_id,
        tenant_name,
        phone_number,
        transaction_type: input.transaction_type,
        status: normalize_optional_text(input.status),
        contract_start: input.contract_start,
        contract_end: input.contract_end,
        rental_amount_cents: input.rental_amount_cents,
        increase_date: input.increase_date,
        increase_rate_basis_points: input.increase_rate_basis_points,
        increase_data: normalize_optional_text(input.increase_data),
        penalty_rate_basis_points: input.penalty_rate_basis_points,
        basic_ele_capacity_centi_kw: input.basic_ele_capacity_centi_kw,
        basic_ele_price_scaled: input.basic_ele_price_scaled,
        area_centi_square_metres: input.area_centi_square_metres,
        remark: normalize_optional_text(input.remark),
        park_id,
        send_message_at: input.send_message_at,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
