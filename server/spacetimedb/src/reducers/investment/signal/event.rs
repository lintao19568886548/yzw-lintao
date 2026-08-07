//! 企业信号事件及其证据的事务维护。

use std::collections::{BTreeMap, BTreeSet};

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::{
    super::{common::limited_optional, profile::require_enterprise_profile},
    common::{require_signal_event, validate_signal_status},
};
use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 企业信号证据可修改字段。
#[derive(SpacetimeType)]
pub struct SignalEvidenceInput {
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

/// 企业信号事件可修改字段，证据在同一事务中整体同步。
#[derive(SpacetimeType)]
pub struct SignalEventInput {
    pub profile_id: Option<u64>,
    pub enterprise_id: Option<u64>,
    pub company_name: String,
    pub event_type: String,
    pub event_title: String,
    pub event_summary: Option<String>,
    pub event_time: Option<Timestamp>,
    pub source_type: String,
    pub source_name: String,
    pub source_url: String,
    pub confidence_score: i32,
    pub status: String,
    pub related_external_lead_id: Option<u64>,
    pub related_radar_lead_id: Option<u64>,
    pub content_hash: String,
    pub raw_payload_json: Option<String>,
    pub evidences: Vec<SignalEvidenceInput>,
}

#[spacetimedb::reducer]
pub fn create_signal_event(ctx: &ReducerContext, input: SignalEventInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let (mut row, evidences) = validated_event(ctx, 0, customer_id.clone(), input)?;
    if let Some(existing) = find_event_by_hash(ctx, &customer_id, &row.content_hash) {
        if !existing.is_deleted {
            return Err("相同内容哈希的企业信号已存在".into());
        }
        row.event_id = existing.event_id;
        row.created_at = existing.created_at;
        row.updated_at = ctx.timestamp;
        sync_evidences(ctx, &row, evidences);
        ctx.db.signal_event().event_id().update(row);
        return Ok(());
    }
    let row = ctx.db.signal_event().insert(row);
    sync_evidences(ctx, &row, evidences);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_signal_event(
    ctx: &ReducerContext,
    event_id: u64,
    input: SignalEventInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let existing = require_signal_event(ctx, event_id)?;
    let (mut row, evidences) = validated_event(ctx, event_id, existing.customer_id.clone(), input)?;
    if find_event_by_hash(ctx, &existing.customer_id, &row.content_hash)
        .is_some_and(|other| other.event_id != event_id && !other.is_deleted)
    {
        return Err("相同内容哈希的企业信号已存在".into());
    }
    row.created_at = existing.created_at;
    row.updated_at = ctx.timestamp;
    sync_evidences(ctx, &row, evidences);
    ctx.db.signal_event().event_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_signal_event_status(
    ctx: &ReducerContext,
    event_id: u64,
    status: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let status = required_text(status, "企业信号状态不能为空")?;
    validate_signal_status(&status)?;
    let mut row = require_signal_event(ctx, event_id)?;
    row.status = status;
    row.updated_at = ctx.timestamp;
    ctx.db.signal_event().event_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_signal_event(ctx: &ReducerContext, event_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut event = require_signal_event(ctx, event_id)?;
    event.is_deleted = true;
    event.updated_at = ctx.timestamp;
    ctx.db.signal_event().event_id().update(event);
    let evidences = ctx
        .db
        .signal_evidence()
        .signal_evidence_by_event()
        .filter(event_id)
        .collect::<Vec<_>>();
    for mut evidence in evidences {
        evidence.is_deleted = true;
        evidence.updated_at = ctx.timestamp;
        ctx.db.signal_evidence().evidence_id().update(evidence);
    }
    Ok(())
}

fn validated_event(
    ctx: &ReducerContext,
    event_id: u64,
    customer_id: String,
    input: SignalEventInput,
) -> Result<(SignalEvent, Vec<SignalEvidence>), String> {
    let company_name = required_text(input.company_name, "企业名称不能为空")?;
    let event_type = required_text(input.event_type, "信号类型不能为空")?;
    let event_title = required_text(input.event_title, "信号标题不能为空")?;
    let source_type = required_text(input.source_type, "来源类型不能为空")?;
    let source_name = required_text(input.source_name, "来源名称不能为空")?;
    let source_url = required_text(input.source_url, "来源链接不能为空")?;
    let status = required_text(input.status, "企业信号状态不能为空")?;
    let content_hash = required_text(input.content_hash, "信号内容哈希不能为空")?;
    validate_max_length(&company_name, 200, "企业名称不能超过200个字符")?;
    validate_max_length(&event_type, 50, "信号类型不能超过50个字符")?;
    validate_max_length(&event_title, 255, "信号标题不能超过255个字符")?;
    validate_max_length(&source_type, 50, "来源类型不能超过50个字符")?;
    validate_max_length(&source_name, 100, "来源名称不能超过100个字符")?;
    validate_max_length(&source_url, 500, "来源链接不能超过500个字符")?;
    validate_max_length(&status, 30, "企业信号状态不能超过30个字符")?;
    validate_max_length(&content_hash, 80, "信号内容哈希不能超过80个字符")?;
    validate_signal_status(&status)?;
    if !(0..=100).contains(&input.confidence_score) {
        return Err("企业信号置信度必须在0到100之间".into());
    }
    let mut enterprise_id = input.enterprise_id;
    if let Some(profile_id) = input.profile_id {
        let profile = require_enterprise_profile(ctx, profile_id)?;
        if profile.company_name != company_name {
            return Err("企业信号名称与关联画像不一致".into());
        }
        if enterprise_id.is_some()
            && profile.enterprise_id.is_some()
            && enterprise_id != profile.enterprise_id
        {
            return Err("企业信号的企业编号与关联画像不一致".into());
        }
        enterprise_id = profile.enterprise_id.or(enterprise_id);
    }
    if let Some(lead_id) = input.related_external_lead_id {
        let valid_lead = ctx
            .db
            .company_lead()
            .lead_id()
            .find(lead_id)
            .is_some_and(|lead| lead.customer_id == customer_id && !lead.is_deleted);
        if !valid_lead {
            return Err("关联的外部企业线索不存在".into());
        }
    }
    let mut hashes = BTreeSet::new();
    let mut evidences = Vec::new();
    for evidence in input.evidences {
        let evidence = validated_evidence(ctx, event_id, customer_id.clone(), evidence)?;
        if !hashes.insert(evidence.content_hash.clone()) {
            return Err("同一企业信号不能包含重复证据".into());
        }
        evidences.push(evidence);
    }
    Ok((
        SignalEvent {
            event_id,
            customer_id,
            profile_id: input.profile_id,
            enterprise_id,
            company_name,
            event_type,
            event_title,
            event_summary: limited_optional(input.event_summary, 65_535, "信号摘要数据过长")?,
            event_time: input.event_time,
            source_type,
            source_name,
            source_url,
            confidence_score: input.confidence_score,
            status,
            related_external_lead_id: input.related_external_lead_id,
            related_radar_lead_id: input.related_radar_lead_id,
            content_hash,
            raw_payload_json: limited_optional(input.raw_payload_json, 65_535, "信号原始数据过长")?,
            is_deleted: false,
            created_at: ctx.timestamp,
            updated_at: ctx.timestamp,
        },
        evidences,
    ))
}

fn validated_evidence(
    ctx: &ReducerContext,
    event_id: u64,
    customer_id: String,
    input: SignalEvidenceInput,
) -> Result<SignalEvidence, String> {
    let evidence_type = required_text(input.evidence_type, "证据类型不能为空")?;
    let source_link = required_text(input.source_link, "证据来源链接不能为空")?;
    let content_hash = required_text(input.content_hash, "证据内容哈希不能为空")?;
    validate_max_length(&evidence_type, 50, "证据类型不能超过50个字符")?;
    validate_max_length(&source_link, 500, "证据来源链接不能超过500个字符")?;
    validate_max_length(&content_hash, 80, "证据内容哈希不能超过80个字符")?;
    Ok(SignalEvidence {
        evidence_id: 0,
        customer_id,
        event_id,
        evidence_type,
        source_title: limited_optional(input.source_title, 255, "证据标题不能超过255个字符")?,
        source_link,
        raw_text: limited_optional(input.raw_text, 65_535, "证据原文数据过长")?,
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

fn find_event_by_hash(
    ctx: &ReducerContext,
    customer_id: &str,
    content_hash: &str,
) -> Option<SignalEvent> {
    ctx.db
        .signal_event()
        .signal_event_by_customer_hash()
        .filter((customer_id, content_hash))
        .next()
}

fn sync_evidences(ctx: &ReducerContext, event: &SignalEvent, rows: Vec<SignalEvidence>) {
    let mut existing = ctx
        .db
        .signal_evidence()
        .signal_evidence_by_event()
        .filter(event.event_id)
        .map(|row| (row.content_hash.clone(), row))
        .collect::<BTreeMap<_, _>>();
    for mut row in rows {
        row.event_id = event.event_id;
        let row_hash = row.content_hash.clone();
        if let Some(old) = existing.remove(&row_hash) {
            row.evidence_id = old.evidence_id;
            row.created_at = old.created_at;
            ctx.db.signal_evidence().evidence_id().update(row);
        } else {
            ctx.db.signal_evidence().insert(row);
        }
    }
    for (_, mut old) in existing {
        old.is_deleted = true;
        old.updated_at = ctx.timestamp;
        ctx.db.signal_evidence().evidence_id().update(old);
    }
}

pub(crate) fn sync_signal_from_company_lead(
    ctx: &ReducerContext,
    lead: &CompanyLead,
    evidences: &[LeadEvidence],
) {
    let existing = ctx
        .db
        .signal_event()
        .signal_event_by_customer()
        .filter(lead.customer_id.as_str())
        .find(|event| event.related_external_lead_id == Some(lead.lead_id));
    let eligible = !lead.is_deleted
        && lead.evidence_count > 0
        && lead.confidence_level != "LOW"
        && lead.confidence_score >= 60;
    if !eligible {
        if let Some(event) = existing {
            soft_delete_event_and_evidences(ctx, event);
        }
        return;
    }

    let profile = ctx
        .db
        .enterprise_profile()
        .enterprise_profile_by_customer_company()
        .filter((lead.customer_id.as_str(), lead.company_name.as_str()))
        .find(|profile| !profile.is_deleted);
    let profile_id = profile.as_ref().map(|profile| profile.profile_id);
    let enterprise_id = profile.as_ref().and_then(|profile| profile.enterprise_id);
    let content_hash = format!("company-lead:{}", lead.lead_id);
    let event_id = existing.as_ref().map_or(0, |event| event.event_id);
    let created_at = existing
        .as_ref()
        .map_or(ctx.timestamp, |event| event.created_at);
    let status = if lead.converted_radar_lead_id.is_some() {
        "CONVERTED".to_string()
    } else if existing.as_ref().is_some_and(|event| !event.is_deleted) {
        existing.as_ref().unwrap().status.clone()
    } else {
        "NEW".to_string()
    };
    let event = SignalEvent {
        event_id,
        customer_id: lead.customer_id.clone(),
        profile_id,
        enterprise_id,
        company_name: lead.company_name.clone(),
        event_type: infer_event_type(lead).to_string(),
        event_title: lead.lead_title.clone(),
        event_summary: lead.summary.clone(),
        event_time: lead.crawled_at.or(lead.last_seen_at),
        source_type: lead.source_type.clone(),
        source_name: lead.source_name.clone(),
        source_url: lead.source_url.clone(),
        confidence_score: lead.confidence_score,
        status,
        related_external_lead_id: Some(lead.lead_id),
        related_radar_lead_id: lead.converted_radar_lead_id,
        content_hash,
        raw_payload_json: lead.hit_keywords_json.clone(),
        is_deleted: false,
        created_at,
        updated_at: ctx.timestamp,
    };
    let event = if event.event_id == 0 {
        ctx.db.signal_event().insert(event)
    } else {
        ctx.db.signal_event().event_id().update(event)
    };
    let signal_evidences = evidences
        .iter()
        .map(|evidence| SignalEvidence {
            evidence_id: 0,
            customer_id: evidence.customer_id.clone(),
            event_id: event.event_id,
            evidence_type: evidence.evidence_type.clone(),
            source_title: evidence.source_title.clone(),
            source_link: evidence.source_link.clone(),
            raw_text: evidence.raw_text.clone(),
            matched_keywords_json: evidence.matched_keywords_json.clone(),
            matched_sentences_json: evidence.matched_sentences_json.clone(),
            score_delta: evidence.score_delta,
            content_hash: evidence.content_hash.clone(),
            published_at: evidence.published_at,
            crawled_at: evidence.crawled_at,
            is_deleted: false,
            created_at: ctx.timestamp,
            updated_at: ctx.timestamp,
        })
        .collect();
    sync_evidences(ctx, &event, signal_evidences);
}

fn infer_event_type(lead: &CompanyLead) -> &'static str {
    let text = format!(
        "{} {} {}",
        lead.demand_type,
        lead.lead_title,
        lead.hit_keywords_json.as_deref().unwrap_or("")
    )
    .to_lowercase();
    if lead.demand_type == "RELOCATION" || text.contains("搬迁") || text.contains("relocation") {
        "RELOCATION"
    } else if lead.demand_type == "RENT_FACTORY"
        || text.contains("租厂")
        || text.contains("厂房")
        || text.contains("factory")
    {
        "FACTORY_RENT_DEMAND"
    } else if text.contains("招聘") || text.contains("recruitment") || text.contains("hiring") {
        "RECRUITMENT_EXPAND"
    } else if text.contains("环评")
        || text.contains("eia")
        || text.contains("扩建")
        || text.contains("新增产线")
        || lead.demand_type == "EXPAND"
        || lead.demand_type == "NEW_LINE"
    {
        "EIA_EXPAND"
    } else if text.contains("公开机会") || text.contains("public") {
        "PUBLIC_FACTORY_DEMAND"
    } else if text.contains("新闻") || text.contains("news") {
        "NEWS_EXPAND"
    } else {
        "UNKNOWN"
    }
}

fn soft_delete_event_and_evidences(ctx: &ReducerContext, mut event: SignalEvent) {
    event.is_deleted = true;
    event.updated_at = ctx.timestamp;
    let event_id = event.event_id;
    ctx.db.signal_event().event_id().update(event);
    let evidences = ctx
        .db
        .signal_evidence()
        .signal_evidence_by_event()
        .filter(event_id)
        .collect::<Vec<_>>();
    for mut evidence in evidences {
        evidence.is_deleted = true;
        evidence.updated_at = ctx.timestamp;
        ctx.db.signal_evidence().evidence_id().update(evidence);
    }
}
