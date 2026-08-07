//! 从原 MySQL 幂等导入账单、财务流水与水电明细。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::shared::access::{AdminContext, current_customer_id},
    tables::{AmountBill, EleBill, Finance, WaterBill, *},
};

use super::sequence::{align_billing_finance_sequence, align_billing_import_sequences};

#[derive(SpacetimeType)]
pub struct MysqlBillingFinanceImport {
    pub finance_id: u64,
    pub bill_name: String,
    pub bill_category: String,
    pub amount_cents: i64,
    pub transaction_type: String,
    pub transaction_time_micros: i64,
    pub remark: Option<String>,
    pub park_id: u64,
    pub status: i32,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlAmountBillImport {
    pub bill_id: u64,
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
    pub receipt_time_micros: Option<i64>,
    pub finance_id: Option<u64>,
    pub tenant_id: u64,
    pub park_id: u64,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlUtilityBillImport {
    pub detail_id: u64,
    pub bill_id: u64,
    pub meter_name: String,
    pub previous_reading_centi: i64,
    pub current_reading_centi: i64,
    pub monthly_usage_centi: i64,
    pub multiplier_centi: i64,
    pub total_usage_centi: i64,
    pub unit_price_scaled: i64,
    pub amount_cents: i64,
    pub remark: Option<String>,
    pub receipt_time_micros: Option<i64>,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlBillingBatchImport {
    pub bills: Vec<MysqlAmountBillImport>,
    pub ele_bills: Vec<MysqlUtilityBillImport>,
    pub water_bills: Vec<MysqlUtilityBillImport>,
}

fn timestamp(value: Option<i64>, fallback: Timestamp) -> Timestamp {
    value
        .map(Timestamp::from_micros_since_unix_epoch)
        .unwrap_or(fallback)
}

fn optional_timestamp(value: Option<i64>) -> Option<Timestamp> {
    value.map(Timestamp::from_micros_since_unix_epoch)
}

fn require_import_park(
    ctx: &ReducerContext,
    customer_id: &str,
    park_id: u64,
) -> Result<(), String> {
    if park_id == 0 {
        return Ok(());
    }
    let park = ctx
        .db
        .park()
        .park_id()
        .find(park_id)
        .ok_or_else(|| format!("园区 {park_id} 尚未迁移"))?;
    if park.customer_id != customer_id {
        return Err(format!("园区 {park_id} 已属于其他租户"));
    }
    Ok(())
}

/// 先导入 MySQL 已存在的财务流水，并推进财务自增序列。
#[spacetimedb::reducer]
pub fn import_mysql_billing_finances(
    ctx: &ReducerContext,
    finances: Vec<MysqlBillingFinanceImport>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    if finances.len() > 5_000 {
        return Err("单次账单财务迁移超过 5000 行安全限制".into());
    }
    for source in finances {
        require_import_park(ctx, &customer_id, source.park_id)?;
        let row = Finance {
            finance_id: source.finance_id,
            customer_id: customer_id.clone(),
            bill_name: source.bill_name,
            bill_category: source.bill_category,
            amount_cents: source.amount_cents,
            transaction_type: source.transaction_type,
            transaction_time: Timestamp::from_micros_since_unix_epoch(
                source.transaction_time_micros,
            ),
            remark: source.remark,
            park_id: source.park_id,
            status: source.status,
            is_deleted: source.is_deleted,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.finance().finance_id().find(row.finance_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("财务流水 {} 已属于其他租户", row.finance_id));
            }
            Some(_) => {
                ctx.db.finance().finance_id().update(row);
            }
            None => {
                ctx.db.finance().insert(row);
            }
        }
    }
    align_billing_finance_sequence(ctx);
    Ok(())
}

fn resolved_finance_id(
    ctx: &ReducerContext,
    customer_id: &str,
    source: &MysqlAmountBillImport,
) -> Result<u64, String> {
    if let Some(finance_id) = source.finance_id {
        let mut finance = ctx
            .db
            .finance()
            .finance_id()
            .find(finance_id)
            .ok_or_else(|| format!("账单 {} 关联财务流水尚未迁移", source.bill_id))?;
        if finance.customer_id != customer_id {
            return Err(format!("财务流水 {finance_id} 已属于其他租户"));
        }
        finance.bill_name = source.project_name.clone();
        finance.bill_category = "账单收入".into();
        finance.amount_cents = source.total_fee_cents;
        finance.transaction_type = "收入".into();
        finance.transaction_time = timestamp(source.receipt_time_micros, finance.created_at);
        finance.remark = source.remark.clone();
        finance.park_id = source.park_id;
        finance.updated_at = optional_timestamp(source.updated_at_micros);
        ctx.db.finance().finance_id().update(finance);
        return Ok(finance_id);
    }

    if let Some(existing) = ctx.db.amount_bill().bill_id().find(source.bill_id) {
        if existing.customer_id != customer_id {
            return Err(format!("账单 {} 已属于其他租户", source.bill_id));
        }
        let mut finance = ctx
            .db
            .finance()
            .finance_id()
            .find(existing.finance_id)
            .ok_or_else(|| format!("账单 {} 的补全财务流水不存在", source.bill_id))?;
        finance.bill_name = source.project_name.clone();
        finance.bill_category = "账单收入".into();
        finance.amount_cents = source.total_fee_cents;
        finance.transaction_type = "收入".into();
        finance.transaction_time = timestamp(source.receipt_time_micros, finance.created_at);
        finance.remark = source.remark.clone();
        finance.park_id = source.park_id;
        finance.updated_at = optional_timestamp(source.updated_at_micros);
        ctx.db.finance().finance_id().update(finance);
        return Ok(existing.finance_id);
    }

    let finance = ctx.db.finance().insert(Finance {
        finance_id: 0,
        customer_id: customer_id.into(),
        bill_name: source.project_name.clone(),
        bill_category: "账单收入".into(),
        amount_cents: source.total_fee_cents,
        transaction_type: "收入".into(),
        transaction_time: timestamp(source.receipt_time_micros, ctx.timestamp),
        remark: source.remark.clone(),
        park_id: source.park_id,
        status: 0,
        is_deleted: false,
        created_at: timestamp(source.created_at_micros, ctx.timestamp),
        updated_at: optional_timestamp(source.updated_at_micros),
    });
    Ok(finance.finance_id)
}

/// 分批导入主账单和水电明细；重复调用会按 MySQL 主键更新同租户记录。
#[spacetimedb::reducer]
pub fn import_mysql_billing_batch(
    ctx: &ReducerContext,
    batch: MysqlBillingBatchImport,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    if batch.bills.len() > 500 || batch.ele_bills.len() > 5_000 || batch.water_bills.len() > 5_000 {
        return Err("单次账单迁移数据量超过安全限制".into());
    }

    for source in batch.bills {
        require_import_park(ctx, &customer_id, source.park_id)?;
        if source.tenant_id != 0 {
            let tenant = ctx
                .db
                .rental_tenant()
                .rental_tenant_id()
                .find(source.tenant_id)
                .ok_or_else(|| format!("租户 {} 尚未迁移", source.tenant_id))?;
            if tenant.customer_id != customer_id {
                return Err(format!("租户 {} 已属于其他客户", source.tenant_id));
            }
        }
        let finance_id = resolved_finance_id(ctx, &customer_id, &source)?;
        let row = AmountBill {
            bill_id: source.bill_id,
            customer_id: customer_id.clone(),
            project_name: source.project_name,
            tenant_name: source.tenant_name,
            public_bank_account: source.public_bank_account,
            private_bank_account: source.private_bank_account,
            ele_fee_cents: source.ele_fee_cents,
            water_fee_cents: source.water_fee_cents,
            receive_fee_cents: source.receive_fee_cents,
            factory_rent_cents: source.factory_rent_cents,
            management_fee_cents: source.management_fee_cents,
            invoice_tax_cents: source.invoice_tax_cents,
            total_fee_cents: source.total_fee_cents,
            service_fee_cents: source.service_fee_cents,
            garbage_fee_cents: source.garbage_fee_cents,
            // 老库没有这几项，迁移过来一律为空／零，由用户在合同里重新约定。
            extra_ele_fee_cents: 0,
            basic_ele_fee_cents: 0,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            receipt_amount_cents: source.receipt_amount_cents,
            penalty_fee_cents: source.penalty_fee_cents,
            service_rate_basis_points: source.service_rate_basis_points,
            garbage_rate_basis_points: source.garbage_rate_basis_points,
            penalty_rate_basis_points: source.penalty_rate_basis_points,
            extra_ele_rate_basis_points: source.extra_ele_rate_basis_points,
            penalty_item: source.penalty_item,
            extra_ele_item: source.extra_ele_item,
            ele_item: source.ele_item,
            water_item: source.water_item,
            extra_project_item: source.extra_project_item,
            tax_rate_json: source.tax_rate_json,
            remark: source.remark,
            receipt_time: optional_timestamp(source.receipt_time_micros),
            finance_id,
            tenant_id: source.tenant_id,
            park_id: source.park_id,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
            // 迁移进来的历史账单先于结转功能存在，恒无结转。
            carryover_fee_cents: 0,
            carryover_item: None,
        };
        match ctx.db.amount_bill().bill_id().find(row.bill_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("账单 {} 已属于其他租户", row.bill_id));
            }
            Some(_) => {
                ctx.db.amount_bill().bill_id().update(row);
            }
            None => {
                ctx.db.amount_bill().insert(row);
            }
        }
    }

    for source in batch.ele_bills {
        let bill = ctx
            .db
            .amount_bill()
            .bill_id()
            .find(source.bill_id)
            .filter(|bill| bill.customer_id == customer_id)
            .ok_or_else(|| format!("电费明细 {} 的账单尚未迁移", source.detail_id))?;
        let row = EleBill {
            ele_id: source.detail_id,
            customer_id: customer_id.clone(),
            bill_id: bill.bill_id,
            // MySQL 侧没有表关联，也没有独立的分时列（时段是塞在表名里的），
            // 迁移过来一律留空，由后续绑定台账时补。
            meter_id: 0,
            tou_tier: None,
            meter_name: source.meter_name,
            previous_reading_centi: source.previous_reading_centi,
            current_reading_centi: source.current_reading_centi,
            monthly_usage_centi: source.monthly_usage_centi,
            multiplier_centi: source.multiplier_centi,
            total_usage_centi: source.total_usage_centi,
            unit_price_scaled: source.unit_price_scaled,
            amount_cents: source.amount_cents,
            remark: source.remark,
            receipt_time: optional_timestamp(source.receipt_time_micros),
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.ele_bill().ele_id().find(row.ele_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("电费明细 {} 已属于其他租户", row.ele_id));
            }
            Some(_) => {
                ctx.db.ele_bill().ele_id().update(row);
            }
            None => {
                ctx.db.ele_bill().insert(row);
            }
        }
    }

    for source in batch.water_bills {
        let bill = ctx
            .db
            .amount_bill()
            .bill_id()
            .find(source.bill_id)
            .filter(|bill| bill.customer_id == customer_id)
            .ok_or_else(|| format!("水费明细 {} 的账单尚未迁移", source.detail_id))?;
        let row = WaterBill {
            water_id: source.detail_id,
            customer_id: customer_id.clone(),
            bill_id: bill.bill_id,
            // 同电费明细：历史数据没有表关联，留 0 等绑定台账时补。
            meter_id: 0,
            meter_name: source.meter_name,
            previous_reading_centi: source.previous_reading_centi,
            current_reading_centi: source.current_reading_centi,
            monthly_usage_centi: source.monthly_usage_centi,
            multiplier_centi: source.multiplier_centi,
            total_usage_centi: source.total_usage_centi,
            unit_price_scaled: source.unit_price_scaled,
            amount_cents: source.amount_cents,
            remark: source.remark,
            receipt_time: optional_timestamp(source.receipt_time_micros),
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.water_bill().water_id().find(row.water_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("水费明细 {} 已属于其他租户", row.water_id));
            }
            Some(_) => {
                ctx.db.water_bill().water_id().update(row);
            }
            None => {
                ctx.db.water_bill().insert(row);
            }
        }
    }
    Ok(())
}

/// 所有批次写入后统一推进账单、水电和财务自增序列。
#[spacetimedb::reducer]
pub fn finalize_mysql_billing_import(ctx: &ReducerContext) -> Result<(), String> {
    AdminContext::require(ctx)?;
    align_billing_import_sequences(ctx);
    Ok(())
}
