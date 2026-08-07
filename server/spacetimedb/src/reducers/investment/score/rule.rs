//! 企业线索评分规则维护。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::super::common::limited_optional;
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 评分规则可修改字段。
#[derive(SpacetimeType)]
pub struct LeadScoreRuleInput {
    pub rule_code: String,
    pub rule_name: String,
    pub event_type: Option<String>,
    pub keywords: Vec<String>,
    pub score_delta: i32,
    pub enabled: bool,
    pub rule_description: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_lead_score_rule(
    ctx: &ReducerContext,
    input: LeadScoreRuleInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let mut row = validated_rule(ctx, 0, customer_id.clone(), input)?;
    if let Some(existing) = find_rule_by_code(ctx, &customer_id, &row.rule_code) {
        if !existing.is_deleted {
            return Err("评分规则编码已存在".into());
        }
        row.rule_id = existing.rule_id;
        row.created_at = existing.created_at;
        row.updated_at = ctx.timestamp;
        ctx.db.lead_score_rule().rule_id().update(row);
        return Ok(());
    }
    ctx.db.lead_score_rule().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_lead_score_rule(
    ctx: &ReducerContext,
    rule_id: u64,
    input: LeadScoreRuleInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_lead_score_rule(ctx, rule_id)?;
    let mut row = validated_rule(ctx, rule_id, existing.customer_id.clone(), input)?;
    if find_rule_by_code(ctx, &existing.customer_id, &row.rule_code)
        .is_some_and(|other| other.rule_id != rule_id && !other.is_deleted)
    {
        return Err("评分规则编码已存在".into());
    }
    row.created_at = existing.created_at;
    row.updated_at = ctx.timestamp;
    ctx.db.lead_score_rule().rule_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_lead_score_rule(ctx: &ReducerContext, rule_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut row = require_lead_score_rule(ctx, rule_id)?;
    row.is_deleted = true;
    row.enabled = false;
    row.updated_at = ctx.timestamp;
    ctx.db.lead_score_rule().rule_id().update(row);
    Ok(())
}

pub(super) fn require_lead_score_rule(
    ctx: &ReducerContext,
    rule_id: u64,
) -> Result<LeadScoreRule, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .lead_score_rule()
        .rule_id()
        .find(rule_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("评分规则不存在".into())
}

fn find_rule_by_code(
    ctx: &ReducerContext,
    customer_id: &str,
    rule_code: &str,
) -> Option<LeadScoreRule> {
    ctx.db
        .lead_score_rule()
        .lead_score_rule_by_customer_code()
        .filter((customer_id, rule_code))
        .next()
}

fn validated_rule(
    ctx: &ReducerContext,
    rule_id: u64,
    customer_id: String,
    input: LeadScoreRuleInput,
) -> Result<LeadScoreRule, String> {
    let rule_code = required_text(input.rule_code, "评分规则编码不能为空")?;
    let rule_name = required_text(input.rule_name, "评分规则名称不能为空")?;
    validate_max_length(&rule_code, 100, "评分规则编码不能超过100个字符")?;
    validate_max_length(&rule_name, 100, "评分规则名称不能超过100个字符")?;
    let mut keywords = BTreeSet::new();
    for keyword in input.keywords {
        let keyword = required_text(keyword, "评分关键词不能为空")?;
        validate_max_length(&keyword, 100, "评分关键词不能超过100个字符")?;
        keywords.insert(keyword);
    }
    Ok(LeadScoreRule {
        rule_id,
        customer_id,
        rule_code,
        rule_name,
        event_type: limited_optional(input.event_type, 50, "事件类型不能超过50个字符")?,
        keywords: keywords.into_iter().collect(),
        score_delta: input.score_delta,
        enabled: input.enabled,
        rule_description: limited_optional(input.rule_description, 65_535, "评分规则说明数据过长")?,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    })
}
