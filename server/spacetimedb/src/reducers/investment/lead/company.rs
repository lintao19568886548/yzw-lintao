//! 外部企业线索、证据及派生信号的事务维护。

use std::collections::{BTreeMap, BTreeSet};

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::super::{common::limited_optional, signal::sync_signal_from_company_lead};
use super::common::{require_company_lead, validate_lead_status};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_user},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 外部企业线索证据可修改字段。
#[derive(SpacetimeType)]
pub struct LeadEvidenceInput {
    pub evidence_type: String,
    pub source_title: Option<String>,
    pub source_link: String,
    pub raw_text: Option<String>,
    pub matched_keywords_json: Option<String>,
    pub matched_sentences_json: Option<String>,
    pub score_delta: i32,
    pub content_hash: String,
    pub published_at: Option<Timestamp>,
    pub crawled_at: Option<Timestamp>,
}

/// 外部企业线索可修改字段，证据在同一事务中整体同步。
#[derive(SpacetimeType)]
pub struct CompanyLeadInput {
    pub source_id: Option<u64>,
    pub source_name: String,
    pub source_url: String,
    pub source_title: Option<String>,
    pub source_type: String,
    pub company_name: String,
    pub lead_title: String,
    pub summary: Option<String>,
    pub demand_type: String,
    pub confidence_score: i32,
    pub confidence_level: String,
    pub industry_name: Option<String>,
    pub region_province: Option<String>,
    pub region_city: Option<String>,
    pub region_district: Option<String>,
    pub hit_keywords_json: Option<String>,
    pub status: String,
    pub owner_user_id: Option<u64>,
    pub invalid_reason: Option<String>,
    pub remark: Option<String>,
    pub dedupe_key: String,
    pub converted_radar_lead_id: Option<u64>,
    pub converted_at: Option<Timestamp>,
    pub first_seen_at: Option<Timestamp>,
    pub last_seen_at: Option<Timestamp>,
    pub crawled_at: Option<Timestamp>,
    pub evidences: Vec<LeadEvidenceInput>,
}

#[spacetimedb::reducer]
pub fn create_company_lead(ctx: &ReducerContext, input: CompanyLeadInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let (mut row, evidences) = validated_lead(ctx, 0, customer_id.clone(), input)?;
    if let Some(existing) = find_lead_by_dedupe(ctx, &customer_id, &row.dedupe_key) {
        if !existing.is_deleted {
            return Err("相同去重键的外部企业线索已存在".into());
        }
        row.lead_id = existing.lead_id;
        row.created_at = existing.created_at;
        row.updated_at = ctx.timestamp;
        let evidences = sync_lead_evidences(ctx, &mut row, evidences);
        sync_signal_from_company_lead(ctx, &row, &evidences);
        ctx.db.company_lead().lead_id().update(row);
        return Ok(());
    }
    let row = ctx.db.company_lead().insert(row);
    let mut row = row;
    let evidences = sync_lead_evidences(ctx, &mut row, evidences);
    sync_signal_from_company_lead(ctx, &row, &evidences);
    ctx.db.company_lead().lead_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_company_lead(
    ctx: &ReducerContext,
    lead_id: u64,
    input: CompanyLeadInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_company_lead(ctx, lead_id)?;
    let (mut row, evidences) = validated_lead(ctx, lead_id, existing.customer_id.clone(), input)?;
    if find_lead_by_dedupe(ctx, &existing.customer_id, &row.dedupe_key)
        .is_some_and(|other| other.lead_id != lead_id && !other.is_deleted)
    {
        return Err("相同去重键的外部企业线索已存在".into());
    }
    row.created_at = existing.created_at;
    row.updated_at = ctx.timestamp;
    row.rule_score = existing.rule_score;
    row.priority_level = existing.priority_level;
    let evidences = sync_lead_evidences(ctx, &mut row, evidences);
    sync_signal_from_company_lead(ctx, &row, &evidences);
    ctx.db.company_lead().lead_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_company_lead(ctx: &ReducerContext, lead_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut lead = require_company_lead(ctx, lead_id)?;
    lead.is_deleted = true;
    lead.updated_at = ctx.timestamp;
    let evidences = active_lead_evidences(ctx, lead_id);
    for mut evidence in evidences {
        evidence.is_deleted = true;
        evidence.updated_at = ctx.timestamp;
        ctx.db.lead_evidence().evidence_id().update(evidence);
    }
    let breakdown_ids = ctx
        .db
        .lead_score_breakdown()
        .lead_score_breakdown_by_lead()
        .filter(lead_id)
        .map(|row| row.breakdown_id)
        .collect::<Vec<_>>();
    for breakdown_id in breakdown_ids {
        ctx.db
            .lead_score_breakdown()
            .breakdown_id()
            .delete(breakdown_id);
    }
    sync_signal_from_company_lead(ctx, &lead, &[]);
    ctx.db.company_lead().lead_id().update(lead);
    Ok(())
}

fn validated_lead(
    ctx: &ReducerContext,
    lead_id: u64,
    customer_id: String,
    input: CompanyLeadInput,
) -> Result<(CompanyLead, Vec<LeadEvidence>), String> {
    let source_name = required_text(input.source_name, "线索来源名称不能为空")?;
    let source_url = required_text(input.source_url, "线索来源链接不能为空")?;
    let source_type = required_text(input.source_type, "线索来源类型不能为空")?;
    let company_name = required_text(input.company_name, "企业名称不能为空")?;
    let lead_title = required_text(input.lead_title, "线索标题不能为空")?;
    let demand_type = required_text(input.demand_type, "需求类型不能为空")?;
    let confidence_level = required_text(input.confidence_level, "置信等级不能为空")?;
    let status = required_text(input.status, "线索状态不能为空")?;
    let dedupe_key = required_text(input.dedupe_key, "线索去重键不能为空")?;
    validate_max_length(&source_name, 100, "线索来源名称不能超过100个字符")?;
    validate_max_length(&source_url, 500, "线索来源链接不能超过500个字符")?;
    validate_max_length(&source_type, 50, "线索来源类型不能超过50个字符")?;
    validate_max_length(&company_name, 200, "企业名称不能超过200个字符")?;
    validate_max_length(&lead_title, 255, "线索标题不能超过255个字符")?;
    validate_max_length(&demand_type, 50, "需求类型不能超过50个字符")?;
    validate_max_length(&confidence_level, 20, "置信等级不能超过20个字符")?;
    validate_max_length(&status, 30, "线索状态不能超过30个字符")?;
    validate_max_length(&dedupe_key, 191, "线索去重键不能超过191个字符")?;
    if ![
        "EXPAND",
        "NEW_LINE",
        "RELOCATION",
        "RENT_FACTORY",
        "UNKNOWN",
    ]
    .contains(&demand_type.as_str())
    {
        return Err("需求类型无效".into());
    }
    if !["HIGH", "LOW", "MEDIUM"].contains(&confidence_level.as_str()) {
        return Err("置信等级无效".into());
    }
    if !(0..=100).contains(&input.confidence_score) {
        return Err("线索置信度必须在0到100之间".into());
    }
    validate_lead_status(&status)?;
    if let Some(user_id) = input.owner_user_id {
        require_user(ctx, user_id)?;
    }
    let mut hashes = BTreeSet::new();
    let mut evidences = Vec::new();
    for evidence in input.evidences {
        let evidence = validated_evidence(ctx, lead_id, customer_id.clone(), evidence)?;
        if !hashes.insert(evidence.content_hash.clone()) {
            return Err("同一外部企业线索不能包含重复证据".into());
        }
        evidences.push(evidence);
    }
    Ok((
        CompanyLead {
            lead_id,
            customer_id,
            source_id: input.source_id,
            source_name,
            source_url,
            source_title: limited_optional(input.source_title, 255, "来源标题不能超过255个字符")?,
            source_type,
            company_name,
            lead_title,
            summary: limited_optional(input.summary, 65_535, "线索摘要数据过长")?,
            demand_type,
            confidence_score: input.confidence_score,
            confidence_level,
            industry_name: limited_optional(input.industry_name, 100, "行业名称不能超过100个字符")?,
            region_province: limited_optional(
                input.region_province,
                100,
                "省份名称不能超过100个字符",
            )?,
            region_city: limited_optional(input.region_city, 100, "城市名称不能超过100个字符")?,
            region_district: limited_optional(
                input.region_district,
                100,
                "区县名称不能超过100个字符",
            )?,
            hit_keywords_json: limited_optional(
                input.hit_keywords_json,
                65_535,
                "命中关键词数据过长",
            )?,
            evidence_count: 0,
            rule_score: 0,
            priority_level: "D".into(),
            status,
            owner_user_id: input.owner_user_id,
            invalid_reason: limited_optional(
                input.invalid_reason,
                255,
                "无效原因不能超过255个字符",
            )?,
            remark: limited_optional(input.remark, 65_535, "线索备注数据过长")?,
            dedupe_key,
            converted_radar_lead_id: input.converted_radar_lead_id,
            converted_at: input.converted_at,
            first_seen_at: input.first_seen_at,
            last_seen_at: input.last_seen_at,
            crawled_at: input.crawled_at,
            is_deleted: false,
            created_at: ctx.timestamp,
            updated_at: ctx.timestamp,
        },
        evidences,
    ))
}

fn validated_evidence(
    ctx: &ReducerContext,
    lead_id: u64,
    customer_id: String,
    input: LeadEvidenceInput,
) -> Result<LeadEvidence, String> {
    let evidence_type = required_text(input.evidence_type, "线索证据类型不能为空")?;
    let source_link = required_text(input.source_link, "线索证据链接不能为空")?;
    let content_hash = required_text(input.content_hash, "线索证据哈希不能为空")?;
    validate_max_length(&evidence_type, 50, "线索证据类型不能超过50个字符")?;
    validate_max_length(&source_link, 500, "线索证据链接不能超过500个字符")?;
    validate_max_length(&content_hash, 80, "线索证据哈希不能超过80个字符")?;
    if !["BODY", "EIA", "NOTICE", "RECRUITMENT", "TITLE"].contains(&evidence_type.as_str()) {
        return Err("线索证据类型无效".into());
    }
    Ok(LeadEvidence {
        evidence_id: 0,
        customer_id,
        lead_id,
        evidence_type,
        source_title: limited_optional(input.source_title, 255, "线索证据标题不能超过255个字符")?,
        source_link,
        raw_text: limited_optional(input.raw_text, 65_535, "线索证据原文数据过长")?,
        matched_keywords_json: limited_optional(
            input.matched_keywords_json,
            65_535,
            "匹配关键词数据过长",
        )?,
        matched_sentences_json: limited_optional(
            input.matched_sentences_json,
            65_535,
            "匹配句子数据过长",
        )?,
        score_delta: input.score_delta,
        content_hash,
        published_at: input.published_at,
        crawled_at: input.crawled_at,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    })
}

fn find_lead_by_dedupe(
    ctx: &ReducerContext,
    customer_id: &str,
    dedupe_key: &str,
) -> Option<CompanyLead> {
    ctx.db
        .company_lead()
        .company_lead_by_customer_dedupe()
        .filter((customer_id, dedupe_key))
        .next()
}

fn sync_lead_evidences(
    ctx: &ReducerContext,
    lead: &mut CompanyLead,
    rows: Vec<LeadEvidence>,
) -> Vec<LeadEvidence> {
    let mut existing = ctx
        .db
        .lead_evidence()
        .lead_evidence_by_lead()
        .filter(lead.lead_id)
        .map(|row| (row.content_hash.clone(), row))
        .collect::<BTreeMap<_, _>>();
    for mut row in rows {
        row.lead_id = lead.lead_id;
        let row_hash = row.content_hash.clone();
        if let Some(old) = existing.remove(&row_hash) {
            row.evidence_id = old.evidence_id;
            row.created_at = old.created_at;
            ctx.db.lead_evidence().evidence_id().update(row);
        } else {
            ctx.db.lead_evidence().insert(row);
        }
    }
    for (_, mut old) in existing {
        old.is_deleted = true;
        old.updated_at = ctx.timestamp;
        ctx.db.lead_evidence().evidence_id().update(old);
    }
    let active = active_lead_evidences(ctx, lead.lead_id);
    lead.evidence_count = active.len() as u64;
    active
}

fn active_lead_evidences(ctx: &ReducerContext, lead_id: u64) -> Vec<LeadEvidence> {
    ctx.db
        .lead_evidence()
        .lead_evidence_by_lead()
        .filter(lead_id)
        .filter(|row| !row.is_deleted)
        .collect()
}
