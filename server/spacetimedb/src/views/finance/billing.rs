//! 当前用户有权访问的应收账单与水电明细。

use spacetimedb::ViewContext;

use crate::views::shared::identity::current_read_scope;
use crate::tables::*;

#[spacetimedb::view(accessor = my_amount_bills, public)]
pub fn my_amount_bills(ctx: &ViewContext) -> Vec<AmountBill> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut bills = ctx
        .db
        .amount_bill()
        .amount_bill_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|bill| scope.allows_park(bill.park_id))
        .collect::<Vec<_>>();
    bills.sort_by_key(|bill| bill.bill_id);
    bills
}

/// 收缴确认记录。园区范围经由所属账单判定——确认表刻意不带 `park_id`。
#[spacetimedb::view(accessor = my_bill_collection_confirmations, public)]
pub fn my_bill_collection_confirmations(ctx: &ViewContext) -> Vec<BillCollectionConfirmation> {
    let mut rows = Vec::new();
    for bill in my_amount_bills(ctx) {
        rows.extend(
            ctx.db
                .bill_collection_confirmation()
                .confirmation_by_bill()
                .filter(bill.bill_id),
        );
    }
    rows.sort_by_key(|row| row.confirmation_id);
    rows
}

#[spacetimedb::view(accessor = my_carryover_batches, public)]
pub fn my_carryover_batches(ctx: &ViewContext) -> Vec<CarryoverBatch> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = Vec::new();
    for park_id in scope.parks() {
        rows.extend(
            ctx.db
                .carryover_batch()
                .carryover_batch_by_park()
                .filter(park_id),
        );
    }
    rows.sort_by_key(|row| row.batch_id);
    rows
}

/// 结转项。园区范围经由所属批次判定——项表刻意不带 `park_id`。
#[spacetimedb::view(accessor = my_carryover_items, public)]
pub fn my_carryover_items(ctx: &ViewContext) -> Vec<CarryoverItem> {
    let mut rows = Vec::new();
    for batch in my_carryover_batches(ctx) {
        rows.extend(
            ctx.db
                .carryover_item()
                .carryover_item_by_batch()
                .filter(batch.batch_id),
        );
    }
    rows.sort_by_key(|row| row.item_id);
    rows
}

#[spacetimedb::view(accessor = my_ele_bills, public)]
pub fn my_ele_bills(ctx: &ViewContext) -> Vec<EleBill> {
    let mut rows = Vec::new();
    for bill in my_amount_bills(ctx) {
        rows.extend(ctx.db.ele_bill().ele_bill_by_bill().filter(bill.bill_id));
    }
    rows.sort_by_key(|row| row.ele_id);
    rows
}

#[spacetimedb::view(accessor = my_water_bills, public)]
pub fn my_water_bills(ctx: &ViewContext) -> Vec<WaterBill> {
    let mut rows = Vec::new();
    for bill in my_amount_bills(ctx) {
        rows.extend(
            ctx.db
                .water_bill()
                .water_bill_by_bill()
                .filter(bill.bill_id),
        );
    }
    rows.sort_by_key(|row| row.water_id);
    rows
}

#[spacetimedb::view(accessor = my_collection_sms_logs, public)]
pub fn my_collection_sms_logs(ctx: &ViewContext) -> Vec<AmountBillCollectionSmsLog> {
    let mut rows = Vec::new();
    for bill in my_amount_bills(ctx) {
        rows.extend(
            ctx.db
                .amount_bill_collection_sms_log()
                .collection_log_by_bill()
                .filter(bill.bill_id),
        );
    }
    rows.sort_by_key(|row| row.log_id);
    rows
}
