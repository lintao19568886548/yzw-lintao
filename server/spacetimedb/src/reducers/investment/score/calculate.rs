//! 根据企业信号重新计算线索评分明细。

use spacetimedb::{ReducerContext, Table};

use super::super::lead::require_company_lead;
use crate::{reducers::shared::access::AdminContext, tables::*};

#[spacetimedb::reducer]
pub fn recalculate_company_lead_score(ctx: &ReducerContext, lead_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut lead = require_company_lead(ctx, lead_id)?;
    let old_ids = ctx
        .db
        .lead_score_breakdown()
        .lead_score_breakdown_by_lead()
        .filter(lead_id)
        .map(|row| row.breakdown_id)
        .collect::<Vec<_>>();
    for breakdown_id in old_ids {
        ctx.db
            .lead_score_breakdown()
            .breakdown_id()
            .delete(breakdown_id);
    }

    let events = ctx
        .db
        .signal_event()
        .signal_event_by_customer()
        .filter(lead.customer_id.as_str())
        .filter(|event| !event.is_deleted && event.related_external_lead_id == Some(lead.lead_id))
        .collect::<Vec<_>>();
    let rules = ctx
        .db
        .lead_score_rule()
        .lead_score_rule_by_customer()
        .filter(lead.customer_id.as_str())
        .filter(|rule| rule.enabled && !rule.is_deleted)
        .collect::<Vec<_>>();
    let mut total = 0_i64;
    for event in events {
        let score_text = signal_score_text(ctx, &event);
        for rule in &rules {
            let reason = if rule.event_type.as_deref() == Some(event.event_type.as_str()) {
                Some(format!(
                    "事件类型 {} 命中：{}",
                    event.event_type, event.event_title
                ))
            } else {
                rule.keywords
                    .iter()
                    .find(|keyword| score_text.contains(&keyword.to_lowercase()))
                    .map(|keyword| format!("关键词“{}”命中：{}", keyword, event.event_title))
            };
            let Some(reason) = reason else {
                continue;
            };
            total += i64::from(rule.score_delta);
            ctx.db.lead_score_breakdown().insert(LeadScoreBreakdown {
                breakdown_id: 0,
                customer_id: lead.customer_id.clone(),
                lead_id: lead.lead_id,
                event_id: Some(event.event_id),
                rule_id: rule.rule_id,
                rule_code: rule.rule_code.clone(),
                rule_name: rule.rule_name.clone(),
                score_delta: rule.score_delta,
                reason,
                created_at: ctx.timestamp,
            });
        }
    }
    lead.rule_score = total.clamp(0, 100) as i32;
    lead.priority_level = priority_level(lead.rule_score).into();
    lead.updated_at = ctx.timestamp;
    ctx.db.company_lead().lead_id().update(lead);
    Ok(())
}

fn signal_score_text(ctx: &ReducerContext, event: &SignalEvent) -> String {
    let mut parts = vec![
        event.company_name.clone(),
        event.event_title.clone(),
        event.event_summary.clone().unwrap_or_default(),
        event.source_url.clone(),
    ];
    for evidence in ctx
        .db
        .signal_evidence()
        .signal_evidence_by_event()
        .filter(event.event_id)
        .filter(|evidence| !evidence.is_deleted)
    {
        parts.push(evidence.raw_text.unwrap_or_default());
        parts.push(evidence.matched_keywords_json.unwrap_or_default());
        parts.push(evidence.matched_sentences_json.unwrap_or_default());
    }
    parts.join(" ").to_lowercase()
}

fn priority_level(score: i32) -> &'static str {
    match score {
        80.. => "A",
        60..=79 => "B",
        40..=59 => "C",
        _ => "D",
    }
}
