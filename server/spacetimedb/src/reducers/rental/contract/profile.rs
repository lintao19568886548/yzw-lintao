//! 独立租户主档事务逻辑。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct TenantProfileInput {
    pub tenant_name: String,
    pub tenant_type: String,
    pub unified_social_credit_code: Option<String>,
    pub legal_representative: Option<String>,
    pub contact_name: String,
    pub phone_number: String,
    pub email: Option<String>,
    pub address: Option<String>,
    pub source: Option<String>,
    pub status: i8,
    pub risk_level: String,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_tenant_profile(
    ctx: &ReducerContext,
    input: TenantProfileInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_profile(ctx, 0, customer_id, input)?;
    ensure_unique_profile(ctx, &row)?;
    ctx.db.tenant_profile().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_tenant_profile(
    ctx: &ReducerContext,
    tenant_profile_id: u64,
    input: TenantProfileInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let existing = ctx
        .db
        .tenant_profile()
        .tenant_profile_id()
        .find(tenant_profile_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("租户档案不存在")?;
    let mut row = validated_profile(ctx, tenant_profile_id, customer_id, input)?;
    ensure_unique_profile(ctx, &row)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_profile().tenant_profile_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_tenant_profile(ctx: &ReducerContext, tenant_profile_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let mut row = ctx
        .db
        .tenant_profile()
        .tenant_profile_id()
        .find(tenant_profile_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("租户档案不存在")?;
    row.is_deleted = true;
    row.status = 0;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.tenant_profile().tenant_profile_id().update(row);
    Ok(())
}

/// 将历史合同中的主体按“名称 + 电话”去重后补入主档，不修改任何合同历史。
#[spacetimedb::reducer]
pub fn sync_tenant_profiles_from_contracts(ctx: &ReducerContext) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let mut existing = ctx
        .db
        .tenant_profile()
        .tenant_profile_by_customer()
        .filter(customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .map(|row| profile_key(&row.tenant_name, &row.phone_number))
        .collect::<BTreeSet<_>>();

    for contract in ctx
        .db
        .rental_tenant()
        .rental_tenant_by_customer()
        .filter(customer_id.as_str())
        .filter(|row| !row.is_deleted)
    {
        let key = profile_key(&contract.tenant_name, &contract.phone_number);
        if !existing.insert(key) {
            continue;
        }
        ctx.db.tenant_profile().insert(TenantProfile {
            tenant_profile_id: 0,
            customer_id: customer_id.clone(),
            tenant_name: contract.tenant_name,
            tenant_type: "enterprise".into(),
            unified_social_credit_code: None,
            legal_representative: None,
            contact_name: String::new(),
            phone_number: contract.phone_number,
            email: None,
            // 合同不再有自由文本地址（租的是哪一层由关联表记录），建档时
            // 留空，通讯地址由客户档案自己维护。
            address: None,
            source: Some("contract_sync".into()),
            status: 1,
            risk_level: "normal".into(),
            remark: Some("由历史合同主体自动建档".into()),
            is_deleted: false,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

fn validated_profile(
    ctx: &ReducerContext,
    tenant_profile_id: u64,
    customer_id: String,
    input: TenantProfileInput,
) -> Result<TenantProfile, String> {
    let tenant_name = required_text(input.tenant_name, "租户名称不能为空")?;
    let phone_number = required_text(input.phone_number, "联系电话不能为空")?;
    let tenant_type = match input.tenant_type.trim() {
        "enterprise" => "enterprise",
        "individual" => "individual",
        _ => return Err("租户类型无效".into()),
    };
    let risk_level = match input.risk_level.trim() {
        "normal" => "normal",
        "watch" => "watch",
        "high" => "high",
        _ => return Err("风险等级无效".into()),
    };
    if !matches!(input.status, 0 | 1) {
        return Err("租户状态无效".into());
    }
    Ok(TenantProfile {
        tenant_profile_id,
        customer_id,
        tenant_name,
        tenant_type: tenant_type.into(),
        unified_social_credit_code: normalize_optional_text(input.unified_social_credit_code),
        legal_representative: normalize_optional_text(input.legal_representative),
        contact_name: input.contact_name.trim().to_string(),
        phone_number,
        email: normalize_optional_text(input.email),
        address: normalize_optional_text(input.address),
        source: normalize_optional_text(input.source),
        status: input.status,
        risk_level: risk_level.into(),
        remark: normalize_optional_text(input.remark),
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

fn ensure_unique_profile(ctx: &ReducerContext, candidate: &TenantProfile) -> Result<(), String> {
    let duplicate = ctx
        .db
        .tenant_profile()
        .tenant_profile_by_customer()
        .filter(candidate.customer_id.as_str())
        .any(|row| {
            !row.is_deleted
                && row.tenant_profile_id != candidate.tenant_profile_id
                && profile_key(&row.tenant_name, &row.phone_number)
                    == profile_key(&candidate.tenant_name, &candidate.phone_number)
        });
    if duplicate {
        return Err("相同名称和联系电话的租户档案已经存在".into());
    }
    Ok(())
}

fn profile_key(name: &str, phone: &str) -> String {
    format!("{}|{}", name.trim().to_lowercase(), phone.trim())
}
