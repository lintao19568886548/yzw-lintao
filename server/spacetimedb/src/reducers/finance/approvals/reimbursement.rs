//! 报销申请、图片关系、审核和财务同步。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{current_customer_id, require_admin, require_image},
        hr::employee_reducers::common::current_enabled_user,
        park_ref::{NO_PARK, required_park_ref},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct ReimbursementInput {
    pub purpose: String,
    pub amount_cents: i64,
    pub payee: String,
    pub reimbursement_date: Option<Timestamp>,
    pub department: Option<String>,
    pub remark: Option<String>,
    pub park_id: u64,
    pub image_ids: Vec<u64>,
}

#[spacetimedb::reducer]
pub fn create_reimbursement(ctx: &ReducerContext, input: ReimbursementInput) -> Result<(), String> {
    create_reimbursement_record(ctx, input).map(|_| ())
}

/// 校验并写入报销主记录和图片关系，供普通创建与 R2 一体化创建共用。
pub(super) fn create_reimbursement_record(
    ctx: &ReducerContext,
    input: ReimbursementInput,
) -> Result<Reimbursement, String> {
    let user = current_enabled_user(ctx)?;
    required_park_ref(ctx, input.park_id)?;
    if !user_can_access_park(ctx, user.id, input.park_id) && require_admin(ctx).is_err() {
        return Err("无该园区申请权限".into());
    }
    if input.amount_cents <= 0 {
        return Err("报销金额必须大于0".into());
    }
    if input.amount_cents > 9_999_999_999 {
        return Err("报销金额超出数据库允许范围".into());
    }
    let purpose = required_text(input.purpose, "报销事由不能为空")?;
    let payee = required_text(input.payee, "收款人不能为空")?;
    validate_max_length(&purpose, 100, "报销事由不能超过100个字符")?;
    validate_max_length(&payee, 50, "收款人不能超过50个字符")?;
    let department = limited_optional(input.department, 50, "部门不能超过50个字符")?;
    let remark = limited_optional(input.remark, 200, "备注不能超过200个字符")?;
    let image_ids = validated_images(ctx, input.image_ids)?;
    let row = ctx.db.reimbursement().insert(Reimbursement {
        id: 0,
        customer_id: user.customer_id,
        purpose,
        amount_cents: input.amount_cents,
        payee,
        reimbursement_date: input.reimbursement_date.unwrap_or(ctx.timestamp),
        department,
        username: Some(user.real_name.clone()),
        remark,
        status: 0,
        is_deleted: false,
        user_id: Some(user.id),
        park_id: input.park_id,
        audit_opinion: None,
        claimant: Some(user.real_name),
        finance_id: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    for img_id in image_ids {
        ctx.db.reimbursement_image().insert(ReimbursementImage {
            id: 0,
            reimbursement_id: row.id,
            img_id,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(row)
}

#[spacetimedb::reducer]
pub fn audit_reimbursement(
    ctx: &ReducerContext,
    id: u64,
    status: i8,
    audit_opinion: Option<String>,
) -> Result<(), String> {
    if !matches!(status, 1 | 2) {
        return Err("审核状态无效".into());
    }
    let auditor = current_enabled_user(ctx)?;
    let mut row = require_reimbursement(ctx, id)?;
    if row.status != 0 {
        return Err("该报销记录已审核，无法重复操作".into());
    }
    let park_id = row.park_id;
    let limit = reimbursement_audit_limit(ctx, auditor.id)?;
    if !user_can_access_park(ctx, auditor.id, park_id) && require_admin(ctx).is_err() {
        return Err("无该园区审核权限".into());
    }
    if let Some(limit_cents) = limit
        && row.amount_cents > limit_cents
    {
        return Err("金额超出审核权限".into());
    }
    let opinion = normalize_optional_text(audit_opinion)
        .map(|value| format!("审核人：{}\n{}", auditor.real_name, value))
        .or_else(|| Some(format!("审核人：{}", auditor.real_name)));
    row.status = status;
    row.audit_opinion = opinion;
    row.updated_at = Some(ctx.timestamp);
    if status == 1 {
        let finance = ctx.db.finance().insert(Finance {
            finance_id: 0,
            customer_id: row.customer_id.clone(),
            bill_name: row.purpose.clone(),
            bill_category: "其他费用".into(),
            amount_cents: row.amount_cents,
            transaction_type: "支出".into(),
            transaction_time: row.created_at,
            remark: Some(format!("报销 #{}", row.id)),
            park_id,
            status: 0,
            is_deleted: false,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        row.finance_id = Some(finance.finance_id);
        copy_images_to_finance(ctx, row.id, finance.finance_id, &row.customer_id);
    }
    ctx.db.reimbursement().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_reimbursement(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let mut row = require_reimbursement(ctx, id)?;
    let is_owner = row.user_id == Some(user.id);
    let can_audit = reimbursement_audit_limit(ctx, user.id).is_ok()
        && row.park_id != NO_PARK
        && user_can_access_park(ctx, user.id, row.park_id);
    if !is_owner && !can_audit && require_admin(ctx).is_err() {
        return Err("无删除该报销记录权限".into());
    }
    row.is_deleted = true;
    row.updated_at = Some(ctx.timestamp);
    if let Some(finance_id) = row.finance_id
        && let Some(mut finance) = ctx.db.finance().finance_id().find(finance_id)
    {
        finance.is_deleted = true;
        finance.updated_at = Some(ctx.timestamp);
        ctx.db.finance().finance_id().update(finance);
    }
    ctx.db.reimbursement().id().update(row);

    // 先解除报销关系，再删除已无任何业务引用的图片元数据；客户端随后才会清理 R2。
    let links = ctx
        .db
        .reimbursement_image()
        .reimbursement_image_by_reimbursement()
        .filter(id)
        .collect::<Vec<_>>();
    for link in &links {
        ctx.db.reimbursement_image().id().delete(link.id);
    }
    for img_id in links.into_iter().map(|link| link.img_id) {
        crate::reducers::platform::media::image_reducer::delete_image_if_unreferenced(ctx, img_id);
    }
    Ok(())
}

fn require_reimbursement(ctx: &ReducerContext, id: u64) -> Result<Reimbursement, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .reimbursement()
        .id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("未找到报销记录".into())
}

fn reimbursement_audit_limit(ctx: &ReducerContext, user_id: u64) -> Result<Option<i64>, String> {
    let roles = ctx
        .db
        .user_role()
        .user_role_by_user()
        .filter(user_id)
        .filter_map(|link| ctx.db.role().role_id().find(link.role_id))
        .filter(|role| role.status == 1)
        .collect::<Vec<_>>();
    if roles
        .iter()
        .any(|role| role.name == "Super" && role.scope == "system")
    {
        return Ok(None);
    }
    let audit_roles = roles
        .into_iter()
        .filter(|role| role.reimbursement_auth.unwrap_or(0) > 0)
        .collect::<Vec<_>>();
    if audit_roles.is_empty() {
        return Err("无报销审核权限".into());
    }
    if audit_roles
        .iter()
        .any(|role| role.rates.is_none_or(|value| value < 0))
    {
        return Ok(None);
    }
    Ok(audit_roles
        .iter()
        .filter_map(|role| role.rates)
        .max()
        .map(|yuan| i64::from(yuan) * 100))
}

fn user_can_access_park(ctx: &ReducerContext, user_id: u64, park_id: u64) -> bool {
    if ctx
        .db
        .user_park()
        .user_park_by_pair()
        .filter((user_id, park_id))
        .any(|link| !link.is_deleted)
    {
        return true;
    }
    ctx.db
        .user_role()
        .user_role_by_user()
        .filter(user_id)
        .filter_map(|link| ctx.db.role().role_id().find(link.role_id))
        .any(|role| {
            role.name == "Super"
                || ctx
                    .db
                    .role_park()
                    .role_park_by_pair()
                    .filter((role.role_id, park_id))
                    .any(|link| !link.is_deleted)
        })
}

fn validated_images(ctx: &ReducerContext, image_ids: Vec<u64>) -> Result<Vec<u64>, String> {
    let mut unique = BTreeSet::new();
    for img_id in image_ids {
        require_image(ctx, img_id)?;
        unique.insert(img_id);
    }
    Ok(unique.into_iter().collect())
}

fn copy_images_to_finance(
    ctx: &ReducerContext,
    reimbursement_id: u64,
    finance_id: u64,
    customer_id: &str,
) {
    for link in ctx
        .db
        .reimbursement_image()
        .reimbursement_image_by_reimbursement()
        .filter(reimbursement_id)
    {
        if let Some(image) = ctx.db.image().img_id().find(link.img_id) {
            ctx.db.finance_image().insert(FinanceImage {
                id: 0,
                customer_id: customer_id.into(),
                finance_id,
                url: image.img_url,
            });
        }
    }
}

fn limited_optional(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(text) = &value {
        validate_max_length(text, max_chars, message)?;
    }
    Ok(value)
}
