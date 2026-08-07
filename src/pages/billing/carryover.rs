//! 账期结转确认页：系统算清单、经理逐张核对、确认后差额才结转。
//!
//! 实现 `docs/租金收缴与对账确认流程.md` §2.2–§2.5。这一页存在的理由是那份
//! 文档里的一句话：**错要错在「没做事」那一侧，不能错在「做错事」那一侧。**
//! 全自动结转在没人点过确认的园区里会把整个账变成假欠款；改成待确认之后，
//! 最坏结果只是「结转没发生」，清单还挂着。
//!
//! 多个账期未确认时**按账期堆叠**（§2.4），不合并成一笔——「拖了几期」正是
//! 长期欠费最重要的预警，合并就把它丢了。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use super::model::{
    carryover_amount_cents, carryover_totals, disposition_label, format_datetime, format_money,
    BATCH_PENDING, DISPOSITION_CARRY, DISPOSITION_COLLECTED, DISPOSITION_SKIP,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        ConfirmDialog,
    },
    services::{
        confirm_carryover_batch_record, discard_carryover_batch_record, open_carryover_batch_record,
        set_carryover_disposition_record,
    },
    spacetime_bindings::{carryover_batch_type::CarryoverBatch, carryover_item_type::CarryoverItem},
    state::WorkspaceState,
};

#[component]
pub fn BillCarryoverPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let parks = state.parks.read().clone();
    let batches = state.carryover_batches.read().clone();
    let items = state.carryover_items.read().clone();
    let bills = state.amount_bills.read().clone();

    let mut feedback = use_signal_sync(|| None::<String>);
    let mut park_id = use_signal(String::new);
    let mut period_label = use_signal(String::new);
    let mut settling = use_signal_sync(|| false);
    let mut confirm_batch = use_signal(|| None::<CarryoverBatch>);
    let mut discard_batch = use_signal(|| None::<CarryoverBatch>);
    let pending_action = use_signal_sync(|| false);

    // 账单编号 → 项目名，清单里要看得出这是哪张账单。
    let bill_names = bills
        .iter()
        .map(|bill| (bill.bill_id, bill.project_name.clone()))
        .collect::<BTreeMap<_, _>>();
    let park_names = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut by_batch = BTreeMap::<u64, Vec<CarryoverItem>>::new();
    for item in &items {
        by_batch.entry(item.batch_id).or_default().push(item.clone());
    }

    let pending_count = batches
        .iter()
        .filter(|batch| batch.status == BATCH_PENDING)
        .count();
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "账期结转" }
                    p { class: "page-subtitle",
                        "账期截止时，未确认收齐的账单由系统算进结转清单。清单只是登记，不改动任何金额——逐张核对并确认之后，差额才并入下一账期。"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            // 多期未确认是运营出问题的强信号，摆在最显眼处（§2.4）。
            if pending_count > 1 {
                p { class: "notice", role: "status",
                    "已有 {pending_count} 个账期的结转未确认。拖期越久，下月账单与实际欠款的偏差越大。"
                }
            }

            section { class: "section",
                Card { CardContent {
                    div { class: "filters",
                        div { class: "field",
                            Label { html_for: "carryover-park", "园区" }
                            Select {
                                id: "carryover-park",
                                value: Some(park_value),
                                on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                                SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                                for (index , park) in parks.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "carryover-park-{park.park_id}",
                                        value: park.park_id.to_string(),
                                        index: index + 1,
                                        text_value: park.park_name.to_string(),
                                        "{park.park_name}"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "carryover-period", "账期标签" }
                            Input {
                                id: "carryover-period",
                                value: period_label(),
                                maxlength: 20,
                                placeholder: "例如 2026-08",
                                oninput: move |event: FormEvent| period_label.set(event.value()),
                            }
                        }
                        div { class: "field",
                            Button {
                                r#type: "button",
                                disabled: settling(),
                                onclick: move |_| {
                                    if settling() {
                                        return;
                                    }
                                    let Ok(park) = park_id().parse::<u64>() else {
                                        feedback.set(Some("请先选择园区".into()));
                                        return;
                                    };
                                    let label = period_label().trim().to_string();
                                    if label.is_empty() {
                                        feedback.set(Some("请填写账期标签".into()));
                                        return;
                                    }
                                    feedback.set(None);
                                    settling.set(true);
                                    spawn(async move {
                                        let result = open_carryover_batch_record(park, label).await;
                                        settling.set(false);
                                        feedback.set(Some(match result {
                                            Ok(()) => "结转清单已算出，请逐张核对后确认".into(),
                                            Err(message) => message,
                                        }));
                                    });
                                },
                                if settling() { "结算中…" } else { "结算本期" }
                            }
                        }
                    }
                    p { class: "hint",
                        "结算会把该园区所有未确认收齐的账单算进一张清单。已经结转过的账单不会重复算入。"
                    }
                } }
            }

            if batches.is_empty() {
                section { class: "section",
                    p { class: "empty",
                        "还没有结转清单。选择园区并填写账期标签后点「结算本期」，系统会算出需要结转的账单。"
                    }
                }
            }

            // 按账期堆叠：每期一张卡，看得出拖了几期（§2.4、§2.7）。
            for batch in batches.iter().cloned() {
                {
                    let batch_items = by_batch.get(&batch.batch_id).cloned().unwrap_or_default();
                    let (carry_count, carry_total) = carryover_totals(&batch_items);
                    let is_pending = batch.status == BATCH_PENDING;
                    let park_name = park_names
                        .get(&batch.park_id)
                        .cloned()
                        .unwrap_or_else(|| "园区已移除".into());
                    let confirm_target = batch.clone();
                    let discard_target = batch.clone();
                    rsx! {
                        section { class: "section", key: "batch-{batch.batch_id}",
                            div { class: "section-header",
                                h2 { "{park_name} · {batch.period_label}" }
                                if is_pending {
                                    Badge { variant: BadgeVariant::Destructive, "待确认" }
                                } else {
                                    Badge { variant: BadgeVariant::Secondary, "已确认" }
                                }
                                Badge { variant: BadgeVariant::Outline,
                                    "结转 {carry_count} 张 · {format_money(carry_total)}"
                                }
                            }
                            if !is_pending {
                                p { class: "hint",
                                    {
                                        let who = batch.confirmed_by_name.clone().unwrap_or_else(|| "--".into());
                                        match batch.confirmed_at {
                                            Some(at) => format!("由 {who} 于 {} 确认", format_datetime(at)),
                                            None => format!("由 {who} 确认"),
                                        }
                                    }
                                }
                            }
                            div { class: "table-shell",
                                table { class: "table",
                                    thead {
                                        tr {
                                            th { "账单" }
                                            th { "应收" }
                                            th { "实收" }
                                            th { "结转额" }
                                            th { "处置" }
                                        }
                                    }
                                    tbody {
                                        if batch_items.is_empty() {
                                            tr { td { class: "table-empty", colspan: "5", "这批没有账单" } }
                                        }
                                        for item in batch_items.iter().cloned() {
                                            {
                                                let name = bill_names
                                                    .get(&item.bill_id)
                                                    .cloned()
                                                    .unwrap_or_else(|| format!("账单 #{}", item.bill_id));
                                                let amount = carryover_amount_cents(&item);
                                                let (label, note) = disposition_label(&item.disposition);
                                                let item_id = item.item_id;
                                                let disposition_value: ReadSignal<Option<String>> = {
                                                    let current = item.disposition.clone();
                                                    use_memo(move || Some(current.clone())).into()
                                                };
                                                rsx! {
                                                    tr { key: "item-{item.item_id}",
                                                        td {
                                                            div { class: "stack-tight",
                                                                strong { "{name}" }
                                                                small { class: "hint", "{note}" }
                                                            }
                                                        }
                                                        td { class: "is-mono", "{format_money(item.receivable_cents)}" }
                                                        td { class: "is-mono", "{format_money(item.received_cents)}" }
                                                        td {
                                                            class: if amount > 0 { "is-mono is-bad" } else { "is-mono" },
                                                            "{format_money(amount)}"
                                                        }
                                                        td {
                                                            if is_pending {
                                                                Select {
                                                                    value: Some(disposition_value),
                                                                    on_value_change: move |value: Option<String>| {
                                                                        let Some(next) = value else { return };
                                                                        let mut feedback = feedback;
                                                                        spawn(async move {
                                                                            if let Err(message) =
                                                                                set_carryover_disposition_record(item_id, next).await
                                                                            {
                                                                                feedback.set(Some(message));
                                                                            }
                                                                        });
                                                                    },
                                                                    SelectOption::<String> { value: DISPOSITION_CARRY.to_string(), index: 0usize, text_value: "结转到下期".to_string(), "结转到下期" }
                                                                    SelectOption::<String> { value: DISPOSITION_SKIP.to_string(), index: 1usize, text_value: "本次不结转".to_string(), "本次不结转" }
                                                                    SelectOption::<String> { value: DISPOSITION_COLLECTED.to_string(), index: 2usize, text_value: "剔除并确认收齐".to_string(), "剔除并确认收齐" }
                                                                }
                                                            } else {
                                                                Badge { variant: BadgeVariant::Outline, "{label}" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if is_pending {
                                div { class: "form-actions",
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        class: "is-quiet-danger",
                                        size: ButtonSize::Sm,
                                        r#type: "button",
                                        onclick: move |_| discard_batch.set(Some(discard_target.clone())),
                                        "丢弃这批"
                                    }
                                    Button {
                                        r#type: "button",
                                        onclick: move |_| confirm_batch.set(Some(confirm_target.clone())),
                                        "确认结转"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                Card { CardContent {
                    p { class: "hint",
                        "「本次不结转」只是本期挂起，下期结算时这张账单还会出现；「剔除并确认收齐」会补一条收齐确认让账单闭环。两者分开是因为语义不同——只想挂起时，系统不该替你记一条并不成立的「已收齐」。"
                    }
                    p { class: "hint",
                        "确认之后清单不可再改。它连同金额快照一起构成对账凭据，说明经理当初核对的是哪些数。"
                    }
                } }
            }
        }

        if let Some(batch) = confirm_batch() {
            {
                let batch_items = by_batch.get(&batch.batch_id).cloned().unwrap_or_default();
                let (carry_count, carry_total) = carryover_totals(&batch_items);
                let collected_count = batch_items
                    .iter()
                    .filter(|item| item.disposition == DISPOSITION_COLLECTED)
                    .count();
                let skip_count = batch_items
                    .iter()
                    .filter(|item| item.disposition == DISPOSITION_SKIP)
                    .count();
                let batch_id = batch.batch_id;
                rsx! {
                    ConfirmDialog {
                        title: "确认本期结转",
                        description: format!(
                            "将结转 {carry_count} 张账单、共 {}；{collected_count} 张剔除并确认收齐；{skip_count} 张本次不结转（下期还会出现）。确认后不可再改。",
                            format_money(carry_total)
                        ),
                        confirm_label: if pending_action() { "确认中…" } else { "确认结转" },
                        on_cancel: move |_| confirm_batch.set(None),
                        on_confirm: move |_| {
                            if pending_action() {
                                return;
                            }
                            let mut feedback = feedback;
                            let mut confirm_batch = confirm_batch;
                            let mut pending_action = pending_action;
                            pending_action.set(true);
                            spawn(async move {
                                let result = confirm_carryover_batch_record(batch_id).await;
                                pending_action.set(false);
                                confirm_batch.set(None);
                                feedback.set(Some(match result {
                                    Ok(()) => "本期结转已确认".into(),
                                    Err(message) => message,
                                }));
                            });
                        },
                    }
                }
            }
        }

        if let Some(batch) = discard_batch() {
            ConfirmDialog {
                title: "丢弃这批结转清单？",
                description: "清单会被删除，账单本身不受影响。下次结算时这些账单会重新算进来。",
                confirm_label: if pending_action() { "处理中…" } else { "确认丢弃" },
                on_cancel: move |_| discard_batch.set(None),
                on_confirm: move |_| {
                    if pending_action() {
                        return;
                    }
                    let batch_id = batch.batch_id;
                    let mut feedback = feedback;
                    let mut discard_batch = discard_batch;
                    let mut pending_action = pending_action;
                    pending_action.set(true);
                    spawn(async move {
                        let result = discard_carryover_batch_record(batch_id).await;
                        pending_action.set(false);
                        discard_batch.set(None);
                        feedback.set(Some(match result {
                            Ok(()) => "清单已丢弃".into(),
                            Err(message) => message,
                        }));
                    });
                },
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn CarryoverRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { BillCarryoverPage {} }
    }

    #[test]
    fn 结转确认页可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { CarryoverRenderTestRoot {} });
        assert!(html.contains("账期结转"), "页面标题没有进入渲染结果");
        assert!(html.contains("结算本期"), "结算入口没有进入渲染结果");
    }
}
