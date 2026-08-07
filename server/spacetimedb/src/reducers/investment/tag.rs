//! 企业画像标签的创建、更新与逻辑删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::profile::require_enterprise_profile;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 企业画像标签可修改字段。
#[derive(SpacetimeType)]
pub struct EnterpriseTagInput {
    pub profile_id: u64,
    pub tag_type: String,
    pub tag_name: String,
    pub tag_source: String,
    pub confidence_score: i32,
}

#[spacetimedb::reducer]
pub fn create_enterprise_tag(
    ctx: &ReducerContext,
    input: EnterpriseTagInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let row = validated_tag(ctx, 0, input)?;
    if let Some(mut existing) = find_tag(ctx, row.profile_id, &row.tag_type, &row.tag_name) {
        if !existing.is_deleted {
            return Err("企业画像标签已存在".into());
        }
        existing.enterprise_id = row.enterprise_id;
        existing.company_name = row.company_name;
        existing.tag_source = row.tag_source;
        existing.confidence_score = row.confidence_score;
        existing.is_deleted = false;
        existing.updated_at = ctx.timestamp;
        ctx.db.enterprise_tag().tag_id().update(existing);
        return Ok(());
    }
    ctx.db.enterprise_tag().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_enterprise_tag(
    ctx: &ReducerContext,
    tag_id: u64,
    input: EnterpriseTagInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_enterprise_tag(ctx, tag_id)?;
    let mut row = validated_tag(ctx, tag_id, input)?;
    if find_tag(ctx, row.profile_id, &row.tag_type, &row.tag_name)
        .is_some_and(|other| other.tag_id != tag_id)
    {
        return Err("企业画像标签已存在".into());
    }
    row.created_at = existing.created_at;
    row.updated_at = ctx.timestamp;
    ctx.db.enterprise_tag().tag_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_enterprise_tag(ctx: &ReducerContext, tag_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut tag = require_enterprise_tag(ctx, tag_id)?;
    tag.is_deleted = true;
    tag.updated_at = ctx.timestamp;
    ctx.db.enterprise_tag().tag_id().update(tag);
    Ok(())
}

fn require_enterprise_tag(ctx: &ReducerContext, tag_id: u64) -> Result<EnterpriseTag, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .enterprise_tag()
        .tag_id()
        .find(tag_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("企业画像标签不存在".into())
}

fn find_tag(
    ctx: &ReducerContext,
    profile_id: u64,
    tag_type: &str,
    tag_name: &str,
) -> Option<EnterpriseTag> {
    ctx.db
        .enterprise_tag()
        .enterprise_tag_by_profile_key()
        .filter((profile_id, tag_type, tag_name))
        .next()
}

fn validated_tag(
    ctx: &ReducerContext,
    tag_id: u64,
    input: EnterpriseTagInput,
) -> Result<EnterpriseTag, String> {
    let profile = require_enterprise_profile(ctx, input.profile_id)?;
    let tag_type = required_text(input.tag_type, "标签类型不能为空")?;
    let tag_name = required_text(input.tag_name, "标签名称不能为空")?;
    let tag_source = required_text(input.tag_source, "标签来源不能为空")?;
    validate_max_length(&tag_type, 50, "标签类型不能超过50个字符")?;
    validate_max_length(&tag_name, 100, "标签名称不能超过100个字符")?;
    validate_max_length(&tag_source, 100, "标签来源不能超过100个字符")?;
    if !(0..=100).contains(&input.confidence_score) {
        return Err("标签置信度必须在0到100之间".into());
    }
    Ok(EnterpriseTag {
        tag_id,
        customer_id: profile.customer_id,
        profile_id: profile.profile_id,
        enterprise_id: profile.enterprise_id,
        company_name: profile.company_name,
        tag_type,
        tag_name,
        tag_source,
        confidence_score: input.confidence_score,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    })
}
