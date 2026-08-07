//! 从原 MySQL 导入工资管理关系链。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::shared::access::{AdminContext, current_customer_id},
    tables::{Image, Park, RentalTenant, SalaryImage, *},
};

use super::sequence::align_salary_import_sequences;

#[derive(SpacetimeType)]
pub struct MysqlParkImport {
    pub park_id: u64,
    pub park_name: String,
    pub address: String,
    pub area_centi_square_metres: i64,
    pub description: Option<String>,
    pub status: Option<String>,
    pub contact: Option<String>,
    pub manager: Option<String>,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlRentalTenantImport {
    pub rental_tenant_id: u64,
    pub tenant_name: String,
    pub phone_number: String,
    pub transaction_type: bool,
    pub status: Option<String>,
    pub contract_start_micros: Option<i64>,
    pub contract_end_micros: Option<i64>,
    pub rental_amount_cents: Option<i64>,
    pub increase_date_micros: Option<i64>,
    pub increase_rate_basis_points: Option<i64>,
    pub increase_data: Option<String>,
    pub penalty_rate_basis_points: Option<i64>,
    pub area_centi_square_metres: Option<i64>,
    pub address: String,
    pub remark: Option<String>,
    pub park_id: u64,
    pub send_message_at_micros: Option<i64>,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlSalaryImport {
    pub salary_id: u64,
    pub rental_tenant_id: u64,
    pub salary_amount_cents: Option<i64>,
    pub issue_date_micros: Option<i64>,
    pub issued: Option<bool>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlImageImport {
    pub img_id: u64,
    pub img_url: String,
    pub hash: String,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlSalaryImageImport {
    pub id: u64,
    pub salary_id: u64,
    pub img_id: u64,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlSalaryBatchImport {
    pub parks: Vec<MysqlParkImport>,
    pub rental_tenants: Vec<MysqlRentalTenantImport>,
    pub salaries: Vec<MysqlSalaryImport>,
    pub images: Vec<MysqlImageImport>,
    pub salary_images: Vec<MysqlSalaryImageImport>,
}

fn timestamp(value: Option<i64>, fallback: Timestamp) -> Timestamp {
    value
        .map(Timestamp::from_micros_since_unix_epoch)
        .unwrap_or(fallback)
}

fn optional_timestamp(value: Option<i64>) -> Option<Timestamp> {
    value.map(Timestamp::from_micros_since_unix_epoch)
}

/// 以原主键幂等导入工资数据；重复调用会更新同租户记录，不会产生重复行。
#[spacetimedb::reducer]
pub fn import_mysql_salary_batch(
    ctx: &ReducerContext,
    batch: MysqlSalaryBatchImport,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;

    for source in batch.parks {
        let row = Park {
            park_id: source.park_id,
            customer_id: customer_id.clone(),
            park_name: source.park_name,
            address: source.address,
            description: source.description,
            status: source.status,
            contact: source.contact,
            manager: source.manager,
            is_deleted: source.is_deleted,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.park().park_id().find(row.park_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("园区 {} 已属于其他租户", row.park_id));
            }
            Some(_) => {
                ctx.db.park().park_id().update(row);
            }
            None => {
                ctx.db.park().insert(row);
            }
        }
    }

    for source in batch.rental_tenants {
        if source.park_id != 0 && ctx.db.park().park_id().find(source.park_id).is_none() {
            return Err(format!("合同人 {} 关联园区不存在", source.rental_tenant_id));
        }
        let row = RentalTenant {
            rental_tenant_id: source.rental_tenant_id,
            customer_id: customer_id.clone(),
            tenant_name: source.tenant_name,
            phone_number: source.phone_number,
            transaction_type: source.transaction_type,
            status: source.status,
            contract_start: optional_timestamp(source.contract_start_micros),
            contract_end: optional_timestamp(source.contract_end_micros),
            rental_amount_cents: source.rental_amount_cents,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            increase_date: optional_timestamp(source.increase_date_micros),
            increase_rate_basis_points: source.increase_rate_basis_points,
            increase_data: source.increase_data,
            penalty_rate_basis_points: source.penalty_rate_basis_points,
            area_centi_square_metres: source.area_centi_square_metres,
            remark: source.remark,
            park_id: source.park_id,
            send_message_at: optional_timestamp(source.send_message_at_micros),
            is_deleted: source.is_deleted,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx
            .db
            .rental_tenant()
            .rental_tenant_id()
            .find(row.rental_tenant_id)
        {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("合同人 {} 已属于其他租户", row.rental_tenant_id));
            }
            Some(_) => {
                ctx.db.rental_tenant().rental_tenant_id().update(row);
            }
            None => {
                ctx.db.rental_tenant().insert(row);
            }
        }
    }

    // 工资不再随这个批次导入。MySQL 里的 `salary.rental_tenant_id` 指向
    // 租赁合同方，而工资已经改为归属员工——照原样导进来只会重建那个错误
    // 模型（页面上出现「给某某有限公司发工资」）。旧工资数据需要人工确认
    // 收款人对应哪位员工之后再补录，没有可靠的自动映射。
    if !batch.salaries.is_empty() {
        return Err(
            "工资已改为按员工归属，MySQL 工资数据挂在合同方下无法自动映射；请清空 salaries 后重试"
                .into(),
        );
    }

    for source in batch.images {
        let row = Image {
            img_id: source.img_id,
            customer_id: customer_id.clone(),
            img_url: source.img_url,
            hash: source.hash,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.image().img_id().find(row.img_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("图片 {} 已属于其他租户", row.img_id));
            }
            Some(_) => {
                ctx.db.image().img_id().update(row);
            }
            None => {
                ctx.db.image().insert(row);
            }
        }
    }

    for source in batch.salary_images {
        if ctx.db.employee_salary().salary_id().find(source.salary_id).is_none()
            || ctx.db.image().img_id().find(source.img_id).is_none()
        {
            return Err(format!("工资图片关系 {} 的父记录不存在", source.id));
        }
        let row = SalaryImage {
            id: source.id,
            customer_id: customer_id.clone(),
            salary_id: source.salary_id,
            img_id: source.img_id,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.salary_image().id().find(row.id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("工资图片关系 {} 已属于其他租户", row.id));
            }
            Some(_) => {
                ctx.db.salary_image().id().update(row);
            }
            None => {
                ctx.db.salary_image().insert(row);
            }
        }
    }

    // 显式写入历史主键不会自动推进 SpacetimeDB 序列，导入后必须校准，
    // 否则页面下一次新增记录可能与 MySQL 历史主键冲突。
    align_salary_import_sequences(ctx);

    Ok(())
}
