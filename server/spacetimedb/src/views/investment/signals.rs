//! 当前用户有权订阅的企业信号和证据。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_signal_events, public)]
pub fn my_signal_events(ctx: &ViewContext) -> Vec<SignalEvent> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .signal_event()
        .signal_event_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.event_time.unwrap_or(row.updated_at));
    rows
}

#[spacetimedb::view(accessor = my_signal_evidences, public)]
pub fn my_signal_evidences(ctx: &ViewContext) -> Vec<SignalEvidence> {
    let event_ids = my_signal_events(ctx)
        .into_iter()
        .map(|row| row.event_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for event_id in event_ids {
        rows.extend(
            ctx.db
                .signal_evidence()
                .signal_evidence_by_event()
                .filter(event_id)
                .filter(|row| !row.is_deleted),
        );
    }
    rows.sort_by_key(|row| row.score_delta);
    rows
}
