//! 历史主键导入后的自增序列校准。

use spacetimedb::{ReducerContext, Table};

use crate::tables::{
    AmountBill, EleBill, Factory, FactoryFloor, FactoryFloorImage, Finance, Image, Park, ParkImage,
    RentalTenant, Salary, SalaryImage, WaterBill, *,
};

/// 校准厂房与楼层历史主键对应的自增序列。
pub(super) fn align_factory_import_sequences(ctx: &ReducerContext) {
    align_factory_sequence(ctx);
    align_factory_floor_sequence(ctx);
}

/// 校准工资关系链涉及的所有自增序列。
///
/// SpacetimeDB 只有在自增字段传入 `0` 时才会消费序列。这里在同一事务内暂存原行、
/// 生成并立即删除占位行，直到新序号越过历史最大主键，然后完整恢复原行。
pub(super) fn align_salary_import_sequences(ctx: &ReducerContext) {
    align_park_sequence(ctx);
    align_rental_tenant_sequence(ctx);
    align_salary_sequence(ctx);
    align_image_sequence(ctx);
    align_salary_image_sequence(ctx);
}

/// 校准园区、楼层图片迁移涉及的自增序列。
pub(super) fn align_media_import_sequences(ctx: &ReducerContext) {
    align_image_sequence(ctx);
    align_park_image_sequence(ctx);
    align_factory_floor_image_sequence(ctx);
}

pub(super) fn align_billing_finance_sequence(ctx: &ReducerContext) {
    align_finance_sequence(ctx);
}

pub(super) fn align_billing_import_sequences(ctx: &ReducerContext) {
    align_finance_sequence(ctx);
    align_amount_bill_sequence(ctx);
    align_ele_bill_sequence(ctx);
    align_water_bill_sequence(ctx);
}

fn align_factory_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.factory().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.factory_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.factory().factory_id().delete(row.factory_id);
    }
    loop {
        let row = ctx.db.factory().insert(Factory {
            factory_id: 0,
            customer_id: "__mysql_sequence__".into(),
            factory_name: "序列校准占位".into(),
            park_id: 0,
            build_date: None,
            description: None,
            is_own: true,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.factory_id;
        ctx.db.factory().factory_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.factory().insert(row);
    }
}

fn align_factory_floor_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.factory_floor().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.floor_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.factory_floor().floor_id().delete(row.floor_id);
    }
    loop {
        let row = ctx.db.factory_floor().insert(FactoryFloor {
            floor_id: 0,
            customer_id: "__mysql_sequence__".into(),
            factory_id: 0,
            floor_name: "序列校准占位".into(),
            floor_height_centi_metres: None,
            load_bearing_centi_units: None,
            rent_price_cents: 0,
            total_area_centi_square_metres: 0,
            description: None,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.floor_id;
        ctx.db.factory_floor().floor_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.factory_floor().insert(row);
    }
}

fn align_park_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.park().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.park_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.park().park_id().delete(row.park_id);
    }
    loop {
        let row = ctx.db.park().insert(Park {
            park_id: 0,
            customer_id: "__mysql_sequence__".into(),
            park_name: "序列校准占位".into(),
            address: String::new(),
            description: None,
            status: None,
            contact: None,
            manager: None,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.park_id;
        ctx.db.park().park_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.park().insert(row);
    }
}

fn align_rental_tenant_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.rental_tenant().iter().collect::<Vec<_>>();
    let max_id = rows
        .iter()
        .map(|row| row.rental_tenant_id)
        .max()
        .unwrap_or(0);
    for row in &rows {
        ctx.db
            .rental_tenant()
            .rental_tenant_id()
            .delete(row.rental_tenant_id);
    }
    loop {
        let row = ctx.db.rental_tenant().insert(RentalTenant {
            rental_tenant_id: 0,
            customer_id: "__mysql_sequence__".into(),
            tenant_name: "序列校准占位".into(),
            phone_number: String::new(),
            transaction_type: false,
            status: None,
            contract_start: None,
            contract_end: None,
            rental_amount_cents: None,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            increase_date: None,
            increase_rate_basis_points: None,
            increase_data: None,
            penalty_rate_basis_points: None,
            area_centi_square_metres: None,
            remark: None,
            park_id: 0,
            send_message_at: None,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.rental_tenant_id;
        ctx.db
            .rental_tenant()
            .rental_tenant_id()
            .delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.rental_tenant().insert(row);
    }
}

fn align_salary_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.employee_salary().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.salary_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.employee_salary().salary_id().delete(row.salary_id);
    }
    loop {
        let row = ctx.db.employee_salary().insert(Salary {
            salary_id: 0,
            customer_id: "__mysql_sequence__".into(),
            employee_id: 0,
            salary_amount_cents: None,
            issue_date: None,
            issued: None,
            remark: None,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.salary_id;
        ctx.db.employee_salary().salary_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.employee_salary().insert(row);
    }
}

fn align_image_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.image().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.img_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.image().img_id().delete(row.img_id);
    }
    loop {
        let row = ctx.db.image().insert(Image {
            img_id: 0,
            customer_id: "__mysql_sequence__".into(),
            img_url: String::new(),
            hash: "__mysql_sequence__".into(),
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.img_id;
        ctx.db.image().img_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.image().insert(row);
    }
}

fn align_park_image_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.park_image().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.park_image().id().delete(row.id);
    }
    loop {
        let row = ctx.db.park_image().insert(ParkImage {
            id: 0,
            customer_id: "__mysql_sequence__".into(),
            park_id: 0,
            img_id: 0,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.id;
        ctx.db.park_image().id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.park_image().insert(row);
    }
}

fn align_factory_floor_image_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.factory_floor_image().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.factory_floor_image().id().delete(row.id);
    }
    loop {
        let row = ctx.db.factory_floor_image().insert(FactoryFloorImage {
            id: 0,
            customer_id: "__mysql_sequence__".into(),
            floor_id: 0,
            img_id: 0,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.id;
        ctx.db.factory_floor_image().id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.factory_floor_image().insert(row);
    }
}

fn align_salary_image_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.salary_image().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.salary_image().id().delete(row.id);
    }
    loop {
        let row = ctx.db.salary_image().insert(SalaryImage {
            id: 0,
            customer_id: "__mysql_sequence__".into(),
            salary_id: 0,
            img_id: 0,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.id;
        ctx.db.salary_image().id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.salary_image().insert(row);
    }
}

fn align_finance_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.finance().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.finance_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.finance().finance_id().delete(row.finance_id);
    }
    loop {
        let row = ctx.db.finance().insert(Finance {
            finance_id: 0,
            customer_id: "__mysql_sequence__".into(),
            bill_name: "序列校准占位".into(),
            bill_category: "迁移".into(),
            amount_cents: 0,
            transaction_type: "收入".into(),
            transaction_time: ctx.timestamp,
            remark: None,
            park_id: 0,
            status: 0,
            is_deleted: true,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.finance_id;
        ctx.db.finance().finance_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.finance().insert(row);
    }
}

fn align_amount_bill_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.amount_bill().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.bill_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.amount_bill().bill_id().delete(row.bill_id);
    }
    loop {
        let row = ctx.db.amount_bill().insert(AmountBill {
            bill_id: 0,
            customer_id: "__mysql_sequence__".into(),
            project_name: "序列校准占位".into(),
            tenant_name: None,
            public_bank_account: None,
            private_bank_account: None,
            ele_fee_cents: 0,
            water_fee_cents: 0,
            receive_fee_cents: 0,
            factory_rent_cents: 0,
            management_fee_cents: 0,
            invoice_tax_cents: 0,
            total_fee_cents: 0,
            service_fee_cents: 0,
            garbage_fee_cents: 0,
            extra_ele_fee_cents: 0,
            basic_ele_fee_cents: 0,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            receipt_amount_cents: 0,
            penalty_fee_cents: None,
            service_rate_basis_points: None,
            garbage_rate_basis_points: None,
            penalty_rate_basis_points: None,
            extra_ele_rate_basis_points: None,
            penalty_item: None,
            extra_ele_item: None,
            ele_item: None,
            water_item: None,
            extra_project_item: None,
            tax_rate_json: None,
            remark: None,
            receipt_time: None,
            finance_id: 0,
            tenant_id: 0,
            park_id: 0,
            created_at: ctx.timestamp,
            updated_at: None,
            // 占位行，与结转无关。
            carryover_fee_cents: 0,
            carryover_item: None,
        });
        let generated_id = row.bill_id;
        ctx.db.amount_bill().bill_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.amount_bill().insert(row);
    }
}

fn align_ele_bill_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.ele_bill().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.ele_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.ele_bill().ele_id().delete(row.ele_id);
    }
    loop {
        let row = ctx.db.ele_bill().insert(EleBill {
            ele_id: 0,
            customer_id: "__mysql_sequence__".into(),
            bill_id: 0,
            meter_id: 0,
            tou_tier: None,
            meter_name: "序列校准占位".into(),
            previous_reading_centi: 0,
            current_reading_centi: 0,
            monthly_usage_centi: 0,
            multiplier_centi: 0,
            total_usage_centi: 0,
            unit_price_scaled: 0,
            amount_cents: 0,
            remark: None,
            receipt_time: None,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.ele_id;
        ctx.db.ele_bill().ele_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.ele_bill().insert(row);
    }
}

fn align_water_bill_sequence(ctx: &ReducerContext) {
    let rows = ctx.db.water_bill().iter().collect::<Vec<_>>();
    let max_id = rows.iter().map(|row| row.water_id).max().unwrap_or(0);
    for row in &rows {
        ctx.db.water_bill().water_id().delete(row.water_id);
    }
    loop {
        let row = ctx.db.water_bill().insert(WaterBill {
            water_id: 0,
            customer_id: "__mysql_sequence__".into(),
            bill_id: 0,
            meter_id: 0,
            meter_name: "序列校准占位".into(),
            previous_reading_centi: 0,
            current_reading_centi: 0,
            monthly_usage_centi: 0,
            multiplier_centi: 0,
            total_usage_centi: 0,
            unit_price_scaled: 0,
            amount_cents: 0,
            remark: None,
            receipt_time: None,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        let generated_id = row.water_id;
        ctx.db.water_bill().water_id().delete(generated_id);
        if generated_id > max_id {
            break;
        }
    }
    for row in rows {
        ctx.db.water_bill().insert(row);
    }
}
