//! 账期结转：结算出清单 → 经理逐张核对 → 确认整批。
//!
//! 实现 `docs/租金收缴与对账确认流程.md` §2.2–§2.5 与第四章的后两个状态。
//! 权限自始至终是**园区管理**，与单张确认一致：谁管这个园区，谁对这个园区的
//! 账负责到底。
//!
//! # 三个动作、一张清单
//!
//! - [`open_carryover_batch`]：把该园区所有「未确认收齐」的账单算进一个新批次。
//!   **只登记不改金额**——这一步做完账面一分钱没动；
//! - [`set_carryover_disposition`]：逐张改处置，取值见 `tables/.../carryover.rs`；
//! - [`confirm_carryover_batch`]：确认整批。`collected` 的补一条收齐确认让账单
//!   闭环，`carry` 的留待并入下期账单，`skip` 的原样放回下期清单。
//!
//! 结算的**触发**故意留给调用方：第一版由经理在界面上点「结算本期」，将来挂
//! 定时任务也是调同一个 Reducer。账单表里没有账期字段，「账期截止」这个时点
//! 只能外部给，硬在这里编一套账期配置反而把判据藏起来。

use spacetimedb::{ReducerContext, Table};

use super::reconciliation::{collection_state, BillFacts, CollectionState};
use crate::{
    reducers::{
        access::{current_customer_id, require_park, require_park_access, require_rental_manager},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 单个批次最多容纳的账单数。
///
/// 结转确认是「看一屏、点一次」的动作，清单长到几百张时经理不可能逐张核对，
/// 那就退化成闭眼确认——比全自动更糟，因为还多了一层「有人看过」的假象。
/// 撞到上限说明该先把积压处理掉。
const MAX_BATCH_ITEMS: usize = 200;

/// 结算：把园区里所有未确认收齐的账单算进一个新批次。
///
/// 不改动任何账单金额，只登记。同一园区同时只允许一个待确认批次。
#[spacetimedb::reducer]
pub fn open_carryover_batch(
    ctx: &ReducerContext,
    park_id: u64,
    period_label: String,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    require_park_access(ctx, park_id)?;
    require_park(ctx, park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let period_label = required_text(period_label, "账期标签不能为空")?;
    validate_max_length(&period_label, 20, "账期标签不能超过20个字符")?;

    // 已有待确认批次时不允许再开：同一张账单进两个批次，确认时会重复结转。
    if pending_batch(ctx, park_id).is_some() {
        return Err("该园区还有未确认的结转清单，请先处理完再结算".into());
    }

    let candidates = carryover_candidates(ctx, &customer_id, park_id);
    if candidates.is_empty() {
        return Err("该园区没有需要结转的账单".into());
    }
    if candidates.len() > MAX_BATCH_ITEMS {
        return Err(format!(
            "待结转账单有 {} 张，超过单批上限 {MAX_BATCH_ITEMS} 张。清单过长时逐张核对形同虚设，请先处理积压",
            candidates.len()
        ));
    }

    let batch_id = ctx
        .db
        .carryover_batch()
        .insert(CarryoverBatch {
            batch_id: 0,
            customer_id: customer_id.clone(),
            park_id,
            period_label,
            status: BATCH_STATUS_PENDING.into(),
            created_at: ctx.timestamp,
            confirmed_by: None,
            confirmed_by_name: None,
            confirmed_at: None,
        })
        .batch_id;
    for bill in candidates {
        ctx.db.carryover_item().insert(CarryoverItem {
            item_id: 0,
            customer_id: customer_id.clone(),
            batch_id,
            bill_id: bill.bill_id,
            receivable_cents: bill.total_fee_cents,
            received_cents: bill.receipt_amount_cents,
            disposition: DISPOSITION_CARRY.into(),
            consumed_by_bill_id: None,
            tenant_id: bill.tenant_id,
            source_project_name: Some(bill.project_name.clone()),
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

/// 逐张改处置。只能改待确认批次里的项。
#[spacetimedb::reducer]
pub fn set_carryover_disposition(
    ctx: &ReducerContext,
    item_id: u64,
    disposition: String,
) -> Result<(), String> {
    require_rental_manager(ctx)?;
    check_disposition(&disposition)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let mut item = ctx
        .db
        .carryover_item()
        .item_id()
        .find(item_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("结转项不存在")?;
    let batch = require_pending_batch(ctx, item.batch_id)?;
    require_park_access(ctx, batch.park_id)?;

    item.disposition = disposition;
    item.updated_at = Some(ctx.timestamp);
    ctx.db.carryover_item().item_id().update(item);
    Ok(())
}

/// 确认整批：`collected` 补收齐确认，`carry` 留待并入下期，`skip` 原样放回。
#[spacetimedb::reducer]
pub fn confirm_carryover_batch(ctx: &ReducerContext, batch_id: u64) -> Result<(), String> {
    let manager_user_id = require_rental_manager(ctx)?;
    let mut batch = require_pending_batch(ctx, batch_id)?;
    require_park_access(ctx, batch.park_id)?;

    let items = ctx
        .db
        .carryover_item()
        .carryover_item_by_batch()
        .filter(batch_id)
        .collect::<Vec<_>>();
    let confirmed_by_name = user_display_name(ctx, manager_user_id);

    for item in &items {
        if item.disposition != DISPOSITION_COLLECTED {
            continue;
        }
        // 剔除并确认收齐：补一条收齐确认让账单闭环。
        //
        // 金额取**账单此刻的实际值**而不是清单里的快照：清单是结算那一刻拍的，
        // 经理之所以要剔除，往往正是因为财务在这中间把实收补录进去了。拿旧快照
        // 落确认，等于记下一个当事人从没看到过的数。
        let Some(bill) = ctx.db.amount_bill().bill_id().find(item.bill_id) else {
            continue;
        };
        if has_collected_confirmation(ctx, item.bill_id) {
            continue;
        }
        ctx.db
            .bill_collection_confirmation()
            .insert(BillCollectionConfirmation {
                confirmation_id: 0,
                customer_id: bill.customer_id.clone(),
                bill_id: bill.bill_id,
                confirm_type: CONFIRM_TYPE_COLLECTED.into(),
                receivable_cents: bill.total_fee_cents,
                received_cents: bill.receipt_amount_cents,
                confirmed_by: manager_user_id,
                confirmed_by_name: confirmed_by_name.clone(),
                remark: Some("结转核对时确认已收齐".into()),
                confirmed_at: ctx.timestamp,
            });
    }

    // `skip` 的项从批次里删掉：它们没有被处置，下次结算会重新算进来。留着只会
    // 让已确认的批次里混着一堆「其实没结转」的行，翻历史的人分不清。
    for item in items.iter().filter(|row| row.disposition == DISPOSITION_SKIP) {
        ctx.db.carryover_item().item_id().delete(item.item_id);
    }

    batch.status = BATCH_STATUS_CONFIRMED.into();
    batch.confirmed_by = Some(manager_user_id);
    batch.confirmed_by_name = Some(confirmed_by_name);
    batch.confirmed_at = Some(ctx.timestamp);
    ctx.db.carryover_batch().batch_id().update(batch);
    Ok(())
}

/// 丢弃一个待确认批次：算错了、或者想换个账期标签重来。
///
/// 只允许丢待确认的——已确认的批次里有收齐确认与结转额，是对账凭据。
#[spacetimedb::reducer]
pub fn discard_carryover_batch(ctx: &ReducerContext, batch_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let batch = require_pending_batch(ctx, batch_id)?;
    require_park_access(ctx, batch.park_id)?;
    let item_ids = ctx
        .db
        .carryover_item()
        .carryover_item_by_batch()
        .filter(batch_id)
        .map(|row| row.item_id)
        .collect::<Vec<_>>();
    for id in item_ids {
        ctx.db.carryover_item().item_id().delete(id);
    }
    ctx.db.carryover_batch().batch_id().delete(batch_id);
    Ok(())
}

/// 该园区待结转的账单：未确认收齐、且不在任何批次里。
///
/// 「不在任何批次里」包含已确认批次——已经结转过的账单不该再被算一次，否则
/// 同一笔差额会被结转多轮，越滚越大。
fn carryover_candidates(
    ctx: &ReducerContext,
    customer_id: &str,
    park_id: u64,
) -> Vec<AmountBill> {
    ctx.db
        .amount_bill()
        .amount_bill_by_park()
        .filter(park_id)
        .filter(|bill| bill.customer_id == customer_id)
        .filter(|bill| {
            let facts = bill_facts(ctx, bill);
            collection_state(&facts) != CollectionState::Closed
        })
        .filter(|bill| {
            ctx.db
                .carryover_item()
                .carryover_item_by_bill()
                .filter(bill.bill_id)
                .next()
                .is_none()
        })
        .collect()
}

fn bill_facts(ctx: &ReducerContext, bill: &AmountBill) -> BillFacts {
    let confirmations = ctx
        .db
        .bill_collection_confirmation()
        .confirmation_by_bill()
        .filter(bill.bill_id)
        .collect::<Vec<_>>();
    BillFacts {
        receivable_cents: bill.total_fee_cents,
        received_cents: bill.receipt_amount_cents,
        has_receipt_time: bill.receipt_time.is_some(),
        has_collected_confirm: confirmations
            .iter()
            .any(|row| row.confirm_type == CONFIRM_TYPE_COLLECTED),
        has_shortfall_confirm: confirmations
            .iter()
            .any(|row| row.confirm_type == CONFIRM_TYPE_SHORTFALL),
    }
}

fn has_collected_confirmation(ctx: &ReducerContext, bill_id: u64) -> bool {
    ctx.db
        .bill_collection_confirmation()
        .confirmation_by_bill()
        .filter(bill_id)
        .any(|row| row.confirm_type == CONFIRM_TYPE_COLLECTED)
}

fn pending_batch(ctx: &ReducerContext, park_id: u64) -> Option<CarryoverBatch> {
    ctx.db
        .carryover_batch()
        .carryover_batch_by_park()
        .filter(park_id)
        .find(|row| row.status == BATCH_STATUS_PENDING)
}

fn require_pending_batch(ctx: &ReducerContext, batch_id: u64) -> Result<CarryoverBatch, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let batch = ctx
        .db
        .carryover_batch()
        .batch_id()
        .find(batch_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("结转清单不存在")?;
    if batch.status != BATCH_STATUS_PENDING {
        return Err("这批结转已经确认过，不能再改".into());
    }
    Ok(batch)
}

fn user_display_name(ctx: &ReducerContext, user_id: u64) -> String {
    ctx.db
        .system_user()
        .id()
        .find(user_id)
        .map(|user| {
            if user.real_name.trim().is_empty() {
                user.username
            } else {
                user.real_name
            }
        })
        .unwrap_or_else(|| format!("用户 #{user_id}"))
}

/// 校验处置取值。
#[pure_function::pure]
pub(crate) fn check_disposition(disposition: &str) -> Result<(), String> {
    if matches!(
        disposition,
        DISPOSITION_CARRY | DISPOSITION_SKIP | DISPOSITION_COLLECTED
    ) {
        Ok(())
    } else {
        Err("处置方式只能是结转、本次不结转或剔除并确认收齐".into())
    }
}

/// 一张结转项要结转多少钱。
///
/// 只有 `carry` 才产生结转额：`skip` 是挂起、`collected` 是认定已收齐，
/// 两者都不该把钱滚到下个月。
#[pure_function::pure]
pub(crate) fn carryover_amount_cents(
    disposition: &str,
    receivable_cents: i64,
    received_cents: i64,
) -> i64 {
    if disposition != DISPOSITION_CARRY {
        return 0;
    }
    receivable_cents.saturating_sub(received_cents).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 处置只接受三个取值() {
        assert!(check_disposition(DISPOSITION_CARRY).is_ok());
        assert!(check_disposition(DISPOSITION_SKIP).is_ok());
        assert!(check_disposition(DISPOSITION_COLLECTED).is_ok());
        assert!(check_disposition("").is_err());
        assert!(check_disposition("结转").is_err());
    }

    #[test]
    fn 只有结转才产生金额() {
        assert_eq!(carryover_amount_cents(DISPOSITION_CARRY, 220_000, 40), 219_960);
        // 挂起和认定已收齐都不该把钱滚到下个月。
        assert_eq!(carryover_amount_cents(DISPOSITION_SKIP, 220_000, 40), 0);
        assert_eq!(carryover_amount_cents(DISPOSITION_COLLECTED, 220_000, 40), 0);
    }

    #[test]
    fn 多缴不产生负数结转() {
        // 实收超过应收时差额为负，结转额必须夹到 0——否则下个月会凭空少收一笔。
        assert_eq!(carryover_amount_cents(DISPOSITION_CARRY, 100_00, 120_00), 0);
    }

    #[test]
    fn 结转额用的是快照不是当前值() {
        // 函数只吃传进来的两个数，不去查库——调用方传快照就是快照，
        // 传当前值就是当前值。这条测试钉住「它不自己去找数据」这个性质。
        assert_eq!(carryover_amount_cents(DISPOSITION_CARRY, 1_000, 300), 700);
        assert_eq!(carryover_amount_cents(DISPOSITION_CARRY, 1_000, 0), 1_000);
    }
}

/// 单张账单一次最多消费的结转项数。
///
/// 与 [`MAX_BATCH_ITEMS`] 同理：能一次勾这么多，说明积压该先清。
const MAX_CONSUMED_ITEMS: usize = 200;

/// 账单声称的结转额，必须等于所消费结转项的差额之和。
///
/// 这是第四步唯一真正要守住的不变式。账单上写「结转 ¥2000」、却勾了合计
/// ¥3000 的结转项，差出来的 ¥1000 就此消失——结转项被标记为已消费，不会再
/// 出现在任何清单里，而账单又没收这笔钱。**钱不会报错，只会不见。**
#[pure_function::pure]
pub(crate) fn check_carryover_consumption(
    claimed_cents: i64,
    shortfalls: &[i64],
) -> Result<(), String> {
    if claimed_cents < 0 {
        return Err("结转金额不能为负数".into());
    }
    let total: i64 = shortfalls.iter().sum();
    if total != claimed_cents {
        return Err(format!(
            "结转金额与所选结转项不符：账单填写 {} 元，所选结转项合计 {} 元。请核对后重新选择",
            claimed_cents as f64 / 100.0,
            total as f64 / 100.0
        ));
    }
    Ok(())
}

/// 取出可被 `bill_id` 消费的结转项，逐项校验。
///
/// `bill_id` 为 0 表示新建账单。编辑既有账单时，已经被这张账单消费的项算
/// 合法——否则改一次账单就要先解绑再重绑，中途任何失败都会留下悬空的结转项。
pub(crate) fn take_consumable_items(
    ctx: &ReducerContext,
    customer_id: &str,
    bill_id: u64,
    item_ids: &[u64],
) -> Result<Vec<CarryoverItem>, String> {
    if item_ids.len() > MAX_CONSUMED_ITEMS {
        return Err(format!(
            "单张账单一次最多并入 {MAX_CONSUMED_ITEMS} 笔结转，请先处理积压"
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut items = Vec::with_capacity(item_ids.len());
    for &item_id in item_ids {
        if !seen.insert(item_id) {
            return Err("同一笔结转不能重复并入一张账单".into());
        }
        let item = ctx
            .db
            .carryover_item()
            .item_id()
            .find(item_id)
            .filter(|row| row.customer_id == customer_id)
            .ok_or("结转记录不存在")?;
        if item.disposition != DISPOSITION_CARRY {
            return Err("只有处置为「结转到下一账期」的记录可以并入账单".into());
        }
        match item.consumed_by_bill_id {
            // 已被别的账单消费：放行就是同一笔差额收两遍。
            Some(owner) if owner != bill_id => {
                return Err("该笔结转已并入其他账单，不能重复并入".into())
            }
            _ => {}
        }
        let batch = ctx
            .db
            .carryover_batch()
            .batch_id()
            .find(item.batch_id)
            .ok_or("结转记录所属批次不存在")?;
        if batch.status != BATCH_STATUS_CONFIRMED {
            return Err("该批结转尚未经园区经理确认，不能并入账单".into());
        }
        items.push(item);
    }
    Ok(items)
}

/// 把结转项标记为被 `bill_id` 消费，并释放本次未再选中的项。
///
/// 与账单写入处在同一个 Reducer，因而同一事务：账单存下来了而标记没落库这种
/// 中间态不存在。**这正是第四步必须动 `create_amount_bill` 入参、而不能另起一个
/// Reducer 的原因**——分两次调用时，第二次失败会让同一笔差额下期被再结转一次。
pub(crate) fn settle_consumption(ctx: &ReducerContext, bill_id: u64, keep: &[CarryoverItem]) {
    let keep_ids = keep.iter().map(|row| row.item_id).collect::<Vec<_>>();
    for mut stale in consumed_items(ctx, bill_id) {
        if keep_ids.contains(&stale.item_id) {
            continue;
        }
        stale.consumed_by_bill_id = None;
        stale.updated_at = Some(ctx.timestamp);
        ctx.db.carryover_item().item_id().update(stale);
    }
    // 按主键重取而不是克隆入参：CarryoverItem 没有 Clone，`item.clone()` 会
    // 悄悄克隆出一个 `&CarryoverItem`，编译不过；重取也顺带避开「入参是校验时
    // 的旧快照」这个隐患。
    for item in keep {
        let Some(mut next) = ctx.db.carryover_item().item_id().find(item.item_id) else {
            continue;
        };
        next.consumed_by_bill_id = Some(bill_id);
        next.updated_at = Some(ctx.timestamp);
        ctx.db.carryover_item().item_id().update(next);
    }
}

/// 账单删除时释放它消费掉的结转项。
///
/// 不释放的话那笔差额就永久消失了：账单没了，结转项却还标着「已并入」，
/// 既不会进新清单，也没有任何账单在收它。
pub(crate) fn release_consumption(ctx: &ReducerContext, bill_id: u64) {
    for mut item in consumed_items(ctx, bill_id) {
        item.consumed_by_bill_id = None;
        item.updated_at = Some(ctx.timestamp);
        ctx.db.carryover_item().item_id().update(item);
    }
}

/// 被某张账单消费的全部结转项。
///
/// 按 `customer_id` 索引扫再过滤，没有为 `consumed_by_bill_id` 单建索引：
/// 结转项是每园区每账期几十条的量级，且这条路径只在账单增删改时走一次。
fn consumed_items(ctx: &ReducerContext, bill_id: u64) -> Vec<CarryoverItem> {
    if bill_id == 0 {
        return Vec::new();
    }
    ctx.db
        .carryover_item()
        .iter()
        .filter(|row| row.consumed_by_bill_id == Some(bill_id))
        .collect()
}

#[cfg(test)]
mod 结转并账校验 {
    use super::*;

    #[test]
    fn 金额与所选结转项一致时通过() {
        assert!(check_carryover_consumption(300_000, &[200_000, 100_000]).is_ok());
    }

    #[test]
    fn 没有结转项时金额必须为零() {
        assert!(check_carryover_consumption(0, &[]).is_ok());
        assert!(check_carryover_consumption(1, &[]).is_err());
    }

    #[test]
    fn 金额少于所选结转项则拒绝() {
        // 差出来的钱会随结转项一起被标记消费，从此不出现在任何清单里。
        let error = check_carryover_consumption(200_000, &[200_000, 100_000]).unwrap_err();
        assert!(error.contains("不符"), "{error}");
    }

    #[test]
    fn 金额多于所选结转项则拒绝() {
        assert!(check_carryover_consumption(400_000, &[200_000, 100_000]).is_err());
    }

    #[test]
    fn 负数金额被拒绝() {
        assert!(check_carryover_consumption(-1, &[]).is_err());
    }
}
