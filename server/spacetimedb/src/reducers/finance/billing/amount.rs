//! 应收账单与财务流水的一对一事务。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::carryover::{
    carryover_amount_cents, check_carryover_consumption, release_consumption, settle_consumption,
    take_consumable_items,
};
use crate::{
    reducers::{
        access::{
            AdminContext, current_customer_id, require_amount_bill, require_rental_tenant,
        },
        park_ref::{has_park, optional_park_ref, NO_PARK},
        rental::contract::tenant_fee::{
            check_agreed_fees, check_basic_ele_fee, collect_bases, MeterScene, SubmittedFees,
        },
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

use super::details::{UtilityBillInput, require_bound_meter, validated_utility};

/// 应收账单可修改字段，金额使用分，费率使用基点。
#[derive(SpacetimeType)]
pub struct AmountBillInput {
    pub project_name: String,
    pub tenant_name: Option<String>,
    pub public_bank_account: Option<String>,
    pub private_bank_account: Option<String>,
    pub ele_fee_cents: i64,
    pub water_fee_cents: i64,
    pub receive_fee_cents: i64,
    pub factory_rent_cents: i64,
    pub management_fee_cents: i64,
    pub invoice_tax_cents: i64,
    pub total_fee_cents: i64,
    pub service_fee_cents: i64,
    pub garbage_fee_cents: i64,
    pub extra_ele_fee_cents: i64,
    pub basic_ele_fee_cents: i64,
    pub basic_ele_capacity_centi_kw: Option<i64>,
    pub basic_ele_price_scaled: Option<i64>,
    pub receipt_amount_cents: i64,
    pub penalty_fee_cents: Option<i64>,
    pub service_rate_basis_points: Option<i64>,
    pub garbage_rate_basis_points: Option<i64>,
    pub penalty_rate_basis_points: Option<i64>,
    pub extra_ele_rate_basis_points: Option<i64>,
    pub penalty_item: Option<String>,
    pub extra_ele_item: Option<String>,
    pub ele_item: Option<String>,
    pub water_item: Option<String>,
    pub extra_project_item: Option<String>,
    pub tax_rate_json: Option<String>,
    pub remark: Option<String>,
    pub receipt_time: Option<Timestamp>,
    pub tenant_id: Option<u64>,
    pub park_id: Option<u64>,
    /// 上期结转过来的欠款（分），单独成项、不并进租金。
    pub carryover_fee_cents: i64,
    /// 结转项名目，如「7月欠款结转」。
    pub carryover_item: Option<String>,
    /// 本次并入的结转项 id。
    ///
    /// 放在账单入参里而不是另起一个 Reducer，是为了让「账单落库」与「结转项
    /// 标记已消费」处在同一个事务：分两次调用时，第二次失败会让同一笔差额
    /// 下期被再结转一次，越滚越大。
    pub consumed_carryover_item_ids: Vec<u64>,
    /// 与账单主记录在同一事务中保存的电表明细。
    pub ele_bills: Vec<UtilityBillInput>,
    /// 与账单主记录在同一事务中保存的水表明细。
    pub water_bills: Vec<UtilityBillInput>,
}

#[spacetimedb::reducer]
pub fn create_amount_bill(ctx: &ReducerContext, mut input: AmountBillInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let details = take_validated_details(ctx, &mut input)?;
    let consumed = take_validated_carryover(ctx, &customer_id, 0, &mut input)?;
    let (mut bill, finance) = validated_bill(ctx, 0, 0, customer_id, input)?;
    let finance = ctx.db.finance().insert(finance);
    bill.finance_id = finance.finance_id;
    let bill = ctx.db.amount_bill().insert(bill);
    replace_bill_details(ctx, &bill, details);
    // 与上面的 insert 同事务：账单存下了而结转项没标记这种中间态不存在。
    settle_consumption(ctx, bill.bill_id, &consumed);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_amount_bill(
    ctx: &ReducerContext,
    bill_id: u64,
    mut input: AmountBillInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_amount_bill(ctx, bill_id)?;
    let finance = ctx
        .db
        .finance()
        .finance_id()
        .find(existing.finance_id)
        .ok_or("账单关联的财务流水不存在")?;
    let details = take_validated_details(ctx, &mut input)?;
    let consumed = take_validated_carryover(ctx, &existing.customer_id, bill_id, &mut input)?;
    let (mut bill, mut next_finance) = validated_bill(
        ctx,
        bill_id,
        existing.finance_id,
        existing.customer_id,
        input,
    )?;
    bill.created_at = existing.created_at;
    bill.updated_at = Some(ctx.timestamp);
    next_finance.created_at = finance.created_at;
    next_finance.updated_at = Some(ctx.timestamp);
    ctx.db.finance().finance_id().update(next_finance);
    let bill = ctx.db.amount_bill().bill_id().update(bill);
    replace_bill_details(ctx, &bill, details);
    // 取消勾选的结转项在这里被释放，重新回到下一期清单。
    settle_consumption(ctx, bill.bill_id, &consumed);
    Ok(())
}

/// 取出并校验本次并入的结转项。
///
/// 两道关：逐项确认可用（属本运营方、处置为结转、批次已确认、未被别的账单
/// 占用），再核对金额之和与账单填写的结转额一致。第二道是关键——账单写着结转
/// ¥2000 却勾了 ¥3000 的项，那 ¥1000 会随结转项一起被标记消费，从此不出现在
/// 任何清单里，也没有任何账单在收它。
fn take_validated_carryover(
    ctx: &ReducerContext,
    customer_id: &str,
    bill_id: u64,
    input: &mut AmountBillInput,
) -> Result<Vec<CarryoverItem>, String> {
    let item_ids = std::mem::take(&mut input.consumed_carryover_item_ids);
    let items = take_consumable_items(ctx, customer_id, bill_id, &item_ids)?;
    // 走 carryover_amount_cents 而不是裸减法：它带 .max(0) 钳位。多缴的账单
    // 同样可能进结转清单（实收超应收时状态是「待确认收齐」，并非已闭环），
    // 裸减法会得到负数，把别的结转项冲掉一块。
    let shortfalls = items
        .iter()
        .map(|row| carryover_amount_cents(&row.disposition, row.receivable_cents, row.received_cents))
        .collect::<Vec<_>>();
    check_carryover_consumption(input.carryover_fee_cents, &shortfalls)?;
    Ok(items)
}

/// 校验并取出水电明细。
///
/// 需要 `ctx` 是因为「明细指向的表是否存在、类型对不对」只能查库——纯字段
/// 校验挡不住"电费明细里填了一块水表"这种串号。
fn take_validated_details(
    ctx: &ReducerContext,
    input: &mut AmountBillInput,
) -> Result<(Vec<UtilityBillInput>, Vec<UtilityBillInput>), String> {
    if input.ele_bills.len() > 100 || input.water_bills.len() > 100 {
        return Err("单份账单的电表或水表明细不能超过 100 行".into());
    }
    let ele_bills = std::mem::take(&mut input.ele_bills)
        .into_iter()
        .map(|row| validated_utility(row, true))
        .collect::<Result<Vec<_>, _>>()?;
    let water_bills = std::mem::take(&mut input.water_bills)
        .into_iter()
        .map(|row| validated_utility(row, false))
        .collect::<Result<Vec<_>, _>>()?;
    // 只要提交了明细，主表的水电合计就以服务端重算结果为准，避免客户端篡改合计。
    if !ele_bills.is_empty() {
        input.ele_fee_cents = ele_bills
            .iter()
            .try_fold(0i64, |total, row| total.checked_add(row.amount_cents))
            .ok_or("电费明细合计数值过大")?;
    }
    if !water_bills.is_empty() {
        input.water_fee_cents = water_bills
            .iter()
            .try_fold(0i64, |total, row| total.checked_add(row.amount_cents))
            .ok_or("水费明细合计数值过大")?;
    }
    for row in &ele_bills {
        require_bound_meter(ctx, row.meter_id, true)?;
    }
    for row in &water_bills {
        require_bound_meter(ctx, row.meter_id, false)?;
    }
    Ok((ele_bills, water_bills))
}

fn replace_bill_details(
    ctx: &ReducerContext,
    bill: &AmountBill,
    (ele_bills, water_bills): (Vec<UtilityBillInput>, Vec<UtilityBillInput>),
) {
    let ele_ids = ctx
        .db
        .ele_bill()
        .ele_bill_by_bill()
        .filter(bill.bill_id)
        .map(|row| row.ele_id)
        .collect::<Vec<_>>();
    for id in ele_ids {
        ctx.db.ele_bill().ele_id().delete(id);
    }
    let water_ids = ctx
        .db
        .water_bill()
        .water_bill_by_bill()
        .filter(bill.bill_id)
        .map(|row| row.water_id)
        .collect::<Vec<_>>();
    for id in water_ids {
        ctx.db.water_bill().water_id().delete(id);
    }

    for input in ele_bills {
        ctx.db.ele_bill().insert(EleBill {
            ele_id: 0,
            customer_id: bill.customer_id.clone(),
            bill_id: bill.bill_id,
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
    }
    for input in water_bills {
        ctx.db.water_bill().insert(WaterBill {
            water_id: 0,
            customer_id: bill.customer_id.clone(),
            bill_id: bill.bill_id,
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
    }
}

#[spacetimedb::reducer]
pub fn delete_amount_bill(ctx: &ReducerContext, bill_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let bill = require_amount_bill(ctx, bill_id)?;
    // 已有收缴确认的账单是被经理认定过的对账凭据，不能删——删了确认记录就
    // 指向一张不存在的账单，快照失去对照对象。建错且已确认的账单走停用或
    // 更正，不走删除。
    if ctx
        .db
        .bill_collection_confirmation()
        .confirmation_by_bill()
        .filter(bill_id)
        .next()
        .is_some()
    {
        return Err("该账单已有收缴确认记录，是对账凭据，不能删除".into());
    }
    let ele_ids = ctx
        .db
        .ele_bill()
        .ele_bill_by_bill()
        .filter(bill_id)
        .map(|row| row.ele_id)
        .collect::<Vec<_>>();
    for id in ele_ids {
        ctx.db.ele_bill().ele_id().delete(id);
    }
    let water_ids = ctx
        .db
        .water_bill()
        .water_bill_by_bill()
        .filter(bill_id)
        .map(|row| row.water_id)
        .collect::<Vec<_>>();
    for id in water_ids {
        ctx.db.water_bill().water_id().delete(id);
    }
    let log_ids = ctx
        .db
        .amount_bill_collection_sms_log()
        .collection_log_by_bill()
        .filter(bill_id)
        .map(|row| row.log_id)
        .collect::<Vec<_>>();
    for id in log_ids {
        ctx.db.amount_bill_collection_sms_log().log_id().delete(id);
    }
    if let Some(mut finance) = ctx.db.finance().finance_id().find(bill.finance_id) {
        let image_ids = ctx
            .db
            .finance_image()
            .finance_image_by_finance()
            .filter(finance.finance_id)
            .map(|image| image.id)
            .collect::<Vec<_>>();
        for id in image_ids {
            ctx.db.finance_image().id().delete(id);
        }
        finance.is_deleted = true;
        finance.updated_at = Some(ctx.timestamp);
        ctx.db.finance().finance_id().update(finance);
    }
    // 释放它并入的结转项：不放的话那笔差额就永久消失——账单没了，结转项却
    // 还标着「已并入」，既不进新清单，也没有账单在收它。
    release_consumption(ctx, bill_id);
    ctx.db.amount_bill().bill_id().delete(bill_id);
    Ok(())
}

fn validated_bill(
    ctx: &ReducerContext,
    bill_id: u64,
    finance_id: u64,
    customer_id: String,
    input: AmountBillInput,
) -> Result<(AmountBill, Finance), String> {
    let tenant_id = input.tenant_id.unwrap_or(0);
    let mut park_id = input.park_id.unwrap_or(NO_PARK);
    if tenant_id != 0 {
        let tenant = require_rental_tenant(ctx, tenant_id)?;
        if has_park(park_id) && has_park(tenant.park_id) && tenant.park_id != park_id {
            return Err("账单园区与租赁客户所属园区不一致".into());
        }
        require_contract_fees(ctx, tenant_id, &input)?;
        if !has_park(park_id) {
            park_id = tenant.park_id;
        }
    }
    // 放在费用核对之后：`input` 要整体借给 `require_contract_fees`，先把
    // `project_name` 移出来的话它就成了部分移动的值。
    let project_name = required_text(input.project_name, "账单项目名称不能为空")?;
    let park_id = optional_park_ref(ctx, park_id)?;
    let amounts = [
        input.ele_fee_cents,
        input.water_fee_cents,
        input.receive_fee_cents,
        input.factory_rent_cents,
        input.management_fee_cents,
        input.invoice_tax_cents,
        input.total_fee_cents,
        input.service_fee_cents,
        input.garbage_fee_cents,
        input.extra_ele_fee_cents,
        input.basic_ele_fee_cents,
        input.receipt_amount_cents,
        input.carryover_fee_cents,
    ];
    if amounts.into_iter().any(|value| value < 0)
        || input.penalty_fee_cents.is_some_and(|value| value < 0)
        || input
        .service_rate_basis_points
        .is_some_and(|value| value < 0)
        || input
        .garbage_rate_basis_points
        .is_some_and(|value| value < 0)
        || input
        .penalty_rate_basis_points
        .is_some_and(|value| value < 0)
        || input
        .extra_ele_rate_basis_points
        .is_some_and(|value| value < 0)
    {
        return Err("账单金额和费率不能为负数".into());
    }
    let transaction_time = input.receipt_time.unwrap_or(ctx.timestamp);
    let bill = AmountBill {
        bill_id,
        customer_id: customer_id.clone(),
        project_name: project_name.clone(),
        tenant_name: normalize_optional_text(input.tenant_name),
        public_bank_account: normalize_optional_text(input.public_bank_account),
        private_bank_account: normalize_optional_text(input.private_bank_account),
        ele_fee_cents: input.ele_fee_cents,
        water_fee_cents: input.water_fee_cents,
        receive_fee_cents: input.receive_fee_cents,
        factory_rent_cents: input.factory_rent_cents,
        management_fee_cents: input.management_fee_cents,
        invoice_tax_cents: input.invoice_tax_cents,
        total_fee_cents: input.total_fee_cents,
        service_fee_cents: input.service_fee_cents,
        garbage_fee_cents: input.garbage_fee_cents,
        extra_ele_fee_cents: input.extra_ele_fee_cents,
        basic_ele_fee_cents: input.basic_ele_fee_cents,
        basic_ele_capacity_centi_kw: input.basic_ele_capacity_centi_kw,
        basic_ele_price_scaled: input.basic_ele_price_scaled,
        receipt_amount_cents: input.receipt_amount_cents,
        penalty_fee_cents: input.penalty_fee_cents,
        service_rate_basis_points: input.service_rate_basis_points,
        garbage_rate_basis_points: input.garbage_rate_basis_points,
        penalty_rate_basis_points: input.penalty_rate_basis_points,
        extra_ele_rate_basis_points: input.extra_ele_rate_basis_points,
        penalty_item: normalize_optional_text(input.penalty_item),
        extra_ele_item: normalize_optional_text(input.extra_ele_item),
        ele_item: normalize_optional_text(input.ele_item),
        water_item: normalize_optional_text(input.water_item),
        extra_project_item: normalize_optional_text(input.extra_project_item),
        tax_rate_json: normalize_optional_text(input.tax_rate_json),
        remark: normalize_optional_text(input.remark),
        receipt_time: input.receipt_time,
        finance_id,
        tenant_id,
        park_id,
        created_at: ctx.timestamp,
        updated_at: None,
        carryover_fee_cents: input.carryover_fee_cents,
        carryover_item: normalize_optional_text(input.carryover_item),
    };
    let finance = Finance {
        finance_id,
        customer_id,
        bill_name: project_name,
        bill_category: "账单收入".into(),
        amount_cents: input.total_fee_cents,
        transaction_type: "收入".into(),
        transaction_time,
        remark: bill.remark.clone(),
        park_id,
        status: 0,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    };
    Ok((bill, finance))
}

/// 按合同约定核对这张账单上的周期性费用和基本电费。
///
/// 基数从**这次一起提交的电费明细**汇总而来，不查库里已存的行：新建账单时库里
/// 还没有明细，编辑时提交的又是改过的版本，两种情况下「当期用量」都只能是入参
/// 里的那一份。
fn require_contract_fees(
    ctx: &ReducerContext,
    tenant_id: u64,
    input: &AmountBillInput,
) -> Result<(), String> {
    check_basic_ele_fee(
        input.basic_ele_capacity_centi_kw,
        input.basic_ele_price_scaled,
        input.basic_ele_fee_cents,
    )?;

    let fees = ctx
        .db
        .rental_tenant_fee()
        .rental_tenant_fee_by_tenant()
        .filter(tenant_id)
        .collect::<Vec<_>>();
    if fees.is_empty() {
        return Ok(());
    }

    let scenes = input.ele_bills.iter().map(|row| {
        // 场景从表台账算出。查不到表（手工补录、`meter_id` 为 0）就算公共区域：
        // 它确实没有安装位置，塞进厂房会让按厂房电费收的电损凭空变大。
        let scene = ctx
            .db
            .utility_meter()
            .meter_id()
            .find(row.meter_id)
            .map(|meter| {
                if meter.factory_floor_id != 0 {
                    MeterScene::Factory
                } else if meter.dormitory_floor_id != 0 {
                    MeterScene::Dormitory
                } else {
                    MeterScene::Public
                }
            })
            .unwrap_or(MeterScene::Public);
        (scene, row.total_usage_centi, row.amount_cents)
    });
    let bases = collect_bases(scenes, input.basic_ele_fee_cents);

    check_agreed_fees(
        &fees,
        &bases,
        SubmittedFees {
            loss_cents: input.extra_ele_fee_cents,
            service_cents: input.service_fee_cents,
            garbage_cents: input.garbage_fee_cents,
        },
    )
}
