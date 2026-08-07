//! 企业画像的创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::limited_optional;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 企业画像可修改字段。
#[derive(SpacetimeType)]
pub struct EnterpriseProfileInput {
    pub enterprise_id: Option<u64>,
    pub company_name: String,
    pub unified_social_credit_code: Option<String>,
    pub industry_name: Option<String>,
    pub industry_tags_json: Option<String>,
    pub region_province: Option<String>,
    pub region_city: Option<String>,
    pub region_district: Option<String>,
    pub registered_capital_cents: Option<i64>,
    pub employee_scale: Option<String>,
    pub business_scope: Option<String>,
    pub address: Option<String>,
    pub last_signal_time: Option<Timestamp>,
    pub signal_count: u64,
    pub latest_intent_type: Option<String>,
    pub profile_completeness: i32,
}

#[spacetimedb::reducer]
pub fn create_enterprise_profile(
    ctx: &ReducerContext,
    input: EnterpriseProfileInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_profile(ctx, 0, customer_id.clone(), input)?;
    if let Some(existing) = find_profile_by_company(ctx, &customer_id, &row.company_name) {
        if !existing.is_deleted {
            return Err("企业画像已存在".into());
        }
        let mut restored = EnterpriseProfile {
            profile_id: existing.profile_id,
            created_at: existing.created_at,
            ..row
        };
        restored.updated_at = ctx.timestamp;
        ctx.db.enterprise_profile().profile_id().update(restored);
        return Ok(());
    }
    ctx.db.enterprise_profile().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_enterprise_profile(
    ctx: &ReducerContext,
    profile_id: u64,
    input: EnterpriseProfileInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_enterprise_profile(ctx, profile_id)?;
    let mut row = validated_profile(ctx, profile_id, existing.customer_id.clone(), input)?;
    if find_profile_by_company(ctx, &existing.customer_id, &row.company_name)
        .is_some_and(|other| other.profile_id != profile_id)
    {
        return Err("企业名称已被其他画像使用".into());
    }
    row.created_at = existing.created_at;
    row.updated_at = ctx.timestamp;
    sync_profile_relation_fields(ctx, &row);
    ctx.db.enterprise_profile().profile_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_enterprise_profile(ctx: &ReducerContext, profile_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut profile = require_enterprise_profile(ctx, profile_id)?;
    profile.is_deleted = true;
    profile.updated_at = ctx.timestamp;
    ctx.db.enterprise_profile().profile_id().update(profile);
    let tags = ctx
        .db
        .enterprise_tag()
        .enterprise_tag_by_profile()
        .filter(profile_id)
        .collect::<Vec<_>>();
    for mut tag in tags {
        tag.is_deleted = true;
        tag.updated_at = ctx.timestamp;
        ctx.db.enterprise_tag().tag_id().update(tag);
    }
    Ok(())
}

pub(super) fn require_enterprise_profile(
    ctx: &ReducerContext,
    profile_id: u64,
) -> Result<EnterpriseProfile, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .enterprise_profile()
        .profile_id()
        .find(profile_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("企业画像不存在".into())
}

fn find_profile_by_company(
    ctx: &ReducerContext,
    customer_id: &str,
    company_name: &str,
) -> Option<EnterpriseProfile> {
    ctx.db
        .enterprise_profile()
        .enterprise_profile_by_customer_company()
        .filter((customer_id, company_name))
        .next()
}

fn sync_profile_relation_fields(ctx: &ReducerContext, profile: &EnterpriseProfile) {
    let tags = ctx
        .db
        .enterprise_tag()
        .enterprise_tag_by_profile()
        .filter(profile.profile_id)
        .collect::<Vec<_>>();
    for mut tag in tags {
        tag.enterprise_id = profile.enterprise_id;
        tag.company_name = profile.company_name.clone();
        tag.updated_at = ctx.timestamp;
        ctx.db.enterprise_tag().tag_id().update(tag);
    }
    let events = ctx
        .db
        .signal_event()
        .signal_event_by_customer()
        .filter(profile.customer_id.as_str())
        .filter(|event| event.profile_id == Some(profile.profile_id))
        .collect::<Vec<_>>();
    for mut event in events {
        event.enterprise_id = profile.enterprise_id;
        event.company_name = profile.company_name.clone();
        event.updated_at = ctx.timestamp;
        ctx.db.signal_event().event_id().update(event);
    }
}

fn validated_profile(
    ctx: &ReducerContext,
    profile_id: u64,
    customer_id: String,
    input: EnterpriseProfileInput,
) -> Result<EnterpriseProfile, String> {
    let company_name = required_text(input.company_name, "企业名称不能为空")?;
    validate_max_length(&company_name, 200, "企业名称不能超过200个字符")?;
    if input
        .registered_capital_cents
        .is_some_and(|value| value < 0)
    {
        return Err("注册资本不能为负数".into());
    }
    if !(0..=100).contains(&input.profile_completeness) {
        return Err("画像完整度必须在0到100之间".into());
    }
    Ok(EnterpriseProfile {
        profile_id,
        customer_id,
        enterprise_id: input.enterprise_id,
        company_name,
        unified_social_credit_code: limited_optional(
            input.unified_social_credit_code,
            100,
            "统一社会信用代码不能超过100个字符",
        )?,
        industry_name: limited_optional(input.industry_name, 100, "行业名称不能超过100个字符")?,
        industry_tags_json: limited_optional(input.industry_tags_json, 16_384, "行业标签数据过长")?,
        region_province: limited_optional(input.region_province, 100, "省份名称不能超过100个字符")?,
        region_city: limited_optional(input.region_city, 100, "城市名称不能超过100个字符")?,
        region_district: limited_optional(input.region_district, 100, "区县名称不能超过100个字符")?,
        registered_capital_cents: input.registered_capital_cents,
        employee_scale: limited_optional(input.employee_scale, 100, "员工规模不能超过100个字符")?,
        business_scope: limited_optional(input.business_scope, 16_384, "经营范围数据过长")?,
        address: limited_optional(input.address, 255, "地址不能超过255个字符")?,
        last_signal_time: input.last_signal_time,
        signal_count: input.signal_count,
        latest_intent_type: limited_optional(
            input.latest_intent_type,
            50,
            "最新意向类型不能超过50个字符",
        )?,
        profile_completeness: input.profile_completeness,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    })
}
