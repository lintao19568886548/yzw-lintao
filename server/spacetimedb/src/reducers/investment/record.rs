//! 招商跟进记录的创建、更新与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{
    limited_optional, require_investment, sync_investment_images, validated_images,
};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        park_ref::{NO_PARK, optional_park_ref},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 招商跟进记录可修改字段。
#[derive(SpacetimeType)]
pub struct InvestmentInput {
    pub agent_name: Option<String>,
    pub tenant_name: Option<String>,
    pub intent_level: String,
    pub intent_area: Option<i64>,
    pub progress: String,
    pub phone_number: Option<String>,
    pub meeting_time: Timestamp,
    pub remark: Option<String>,
    pub park_id: Option<u64>,
    pub image_ids: Vec<u64>,
}

#[spacetimedb::reducer]
pub fn create_investment(ctx: &ReducerContext, input: InvestmentInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let image_ids = validated_images(ctx, input.image_ids.clone())?;
    let row = validated_row(ctx, 0, customer_id, input)?;
    let row = ctx.db.investment().insert(row);
    sync_investment_images(ctx, row.investment_id, image_ids);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_investment(
    ctx: &ReducerContext,
    investment_id: u64,
    input: InvestmentInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_investment(ctx, investment_id)?;
    let image_ids = validated_images(ctx, input.image_ids.clone())?;
    let mut row = validated_row(ctx, investment_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.investment().investment_id().update(row);
    sync_investment_images(ctx, investment_id, image_ids);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_investment(ctx: &ReducerContext, investment_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_investment(ctx, investment_id)?;
    sync_investment_images(ctx, investment_id, Vec::new());
    ctx.db.investment().investment_id().delete(investment_id);
    Ok(())
}

fn validated_row(
    ctx: &ReducerContext,
    investment_id: u64,
    customer_id: String,
    input: InvestmentInput,
) -> Result<Investment, String> {
    let intent_level = required_text(input.intent_level, "意向等级不能为空")?;
    let progress = required_text(input.progress, "跟进进度不能为空")?;
    validate_max_length(&intent_level, 50, "意向等级不能超过50个字符")?;
    validate_max_length(&progress, 30, "跟进进度不能超过30个字符")?;
    if input.intent_area.is_some_and(|value| value < 0) {
        return Err("意向面积不能为负数".into());
    }
    let park_id = optional_park_ref(ctx, input.park_id.unwrap_or(NO_PARK))?;
    Ok(Investment {
        investment_id,
        customer_id,
        agent_name: limited_optional(input.agent_name, 100, "招商人员名称不能超过100个字符")?,
        tenant_name: limited_optional(input.tenant_name, 100, "意向客户名称不能超过100个字符")?,
        intent_level,
        intent_area: input.intent_area,
        progress,
        phone_number: limited_optional(input.phone_number, 11, "联系电话不能超过11个字符")?,
        meeting_time: input.meeting_time,
        remark: limited_optional(input.remark, 100, "备注不能超过100个字符")?,
        park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
