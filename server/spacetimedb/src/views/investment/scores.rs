//! 当前用户有权订阅的评分规则与评分明细。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

use super::leads::my_company_leads;

#[spacetimedb::view(accessor = my_lead_score_rules, public)]
pub fn my_lead_score_rules(ctx: &ViewContext) -> Vec<LeadScoreRule> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .lead_score_rule()
        .lead_score_rule_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.updated_at);
    rows
}

#[spacetimedb::view(accessor = my_lead_score_breakdowns, public)]
pub fn my_lead_score_breakdowns(ctx: &ViewContext) -> Vec<LeadScoreBreakdown> {
    let lead_ids = my_company_leads(ctx)
        .into_iter()
        .map(|lead| lead.lead_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for lead_id in lead_ids {
        rows.extend(
            ctx.db
                .lead_score_breakdown()
                .lead_score_breakdown_by_lead()
                .filter(lead_id),
        );
    }
    rows.sort_by_key(|row| row.created_at);
    rows
}
