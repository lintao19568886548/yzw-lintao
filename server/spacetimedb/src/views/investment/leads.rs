//! 当前用户有权订阅的外部企业线索和证据。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_company_leads, public)]
pub fn my_company_leads(ctx: &ViewContext) -> Vec<CompanyLead> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .company_lead()
        .company_lead_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.updated_at);
    rows
}

#[spacetimedb::view(accessor = my_lead_evidences, public)]
pub fn my_lead_evidences(ctx: &ViewContext) -> Vec<LeadEvidence> {
    let lead_ids = my_company_leads(ctx)
        .into_iter()
        .map(|row| row.lead_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for lead_id in lead_ids {
        rows.extend(
            ctx.db
                .lead_evidence()
                .lead_evidence_by_lead()
                .filter(lead_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.score_delta);
    rows
}
