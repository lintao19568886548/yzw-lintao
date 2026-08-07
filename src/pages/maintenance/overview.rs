//! 报修工单台账页面。
//!
//! 消防、变压器、电梯都已改为「资产 + 巡检」两表分离的模型（分别见
//! `firefighting.rs` / `transformer.rs` / `elevator.rs` 与 docs/ 第四至六篇）；
//! 工单是流程而不是资产，保留记录流水模型：接单 → 完工 → 验收闭环，
//! 退回与取消是分支。

use dioxus::prelude::*;

use super::model::*;
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        total_pages, ConfirmDialog, Pager,
    },
    services::{delete_repair_order_record, save_repair_order, transition_repair_order_record},
    spacetime_bindings::repair_order_input_type::RepairOrderInput,
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

#[derive(Clone, PartialEq)]
struct ViewRow {
    id: u64,
    code: String,
    title: String,
    location: String,
    status: String,
    priority: String,
    summary: String,
    operator: String,
    checked_at: String,
}

#[derive(Clone, PartialEq)]
struct OrderDraft {
    id: Option<u64>,
    park_id: String,
    factory_id: String,
    source: String,
    repair_type: String,
    priority: String,
    tenant_name: String,
    tenant_phone: String,
    assignee: String,
    assignee_phone: String,
    description: String,
}

impl OrderDraft {
    fn fresh(state: WorkspaceState) -> Self {
        let park_id = state
            .parks
            .read()
            .first()
            .map(|row| row.park_id.to_string())
            .unwrap_or_default();
        let factory_id = state
            .factories
            .read()
            .iter()
            .find(|row| row.park_id.to_string() == park_id)
            .map(|row| row.factory_id.to_string())
            .unwrap_or_default();
        Self {
            id: None,
            park_id,
            factory_id,
            source: "物业代报修".into(),
            repair_type: "设备".into(),
            priority: "普通".into(),
            tenant_name: String::new(),
            tenant_phone: String::new(),
            assignee: String::new(),
            assignee_phone: String::new(),
            description: String::new(),
        }
    }
}

fn optional(value: String) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

fn view_rows(state: WorkspaceState) -> Vec<ViewRow> {
    let parks = state.parks.read();
    let factories = state.factories.read();
    state
        .repair_orders
        .read()
        .iter()
        .map(|row| ViewRow {
            id: row.repair_order_id,
            code: row.order_no.clone(),
            title: format!(
                "{} · {}",
                row.repair_type,
                row.tenant_name.clone().unwrap_or_else(|| "物业报修".into())
            ),
            location: format!(
                "{} · {}",
                park_name(&parks, row.park_id),
                factory_name(&factories, row.factory_id)
            ),
            status: row.status.clone(),
            priority: row.priority.clone(),
            summary: row.description.clone(),
            operator: row.assignee.clone().unwrap_or_else(|| "待分派".into()),
            checked_at: format_datetime(row.created_at),
        })
        .collect()
}

fn draft_for(id: u64, state: WorkspaceState) -> Option<OrderDraft> {
    let row = state
        .repair_orders
        .read()
        .iter()
        .find(|row| row.repair_order_id == id)?
        .clone();
    let mut draft = OrderDraft::fresh(state);
    draft.id = Some(id);
    draft.park_id = row.park_id.to_string();
    draft.factory_id = row.factory_id.map(|v| v.to_string()).unwrap_or_default();
    draft.source = row.source;
    draft.repair_type = row.repair_type;
    draft.priority = row.priority;
    draft.tenant_name = row.tenant_name.unwrap_or_default();
    draft.tenant_phone = row.tenant_phone.unwrap_or_default();
    draft.assignee = row.assignee.unwrap_or_default();
    draft.assignee_phone = row.assignee_phone.unwrap_or_default();
    draft.description = row.description;
    Some(draft)
}

#[component]
pub fn MaintenanceRepairOrderPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let mut query = use_signal(String::new);
    let mut status_filter = use_signal(|| "全部".to_string());
    let mut form_open = use_signal_sync(|| false);
    let mut draft = use_signal_sync(|| OrderDraft::fresh(state));
    let mut saving = use_signal_sync(|| false);
    let mut feedback = use_signal_sync(|| None::<String>);
    let mut confirm_delete = use_signal(|| None::<u64>);
    let mut workflow = use_signal(|| None::<(u64, String, String)>);
    let mut workflow_remark = use_signal(String::new);
    let mut page = use_signal(|| 1usize);
    let all_rows = view_rows(state);
    let normal_count = all_rows
        .iter()
        .filter(|row| matches!(row.status.as_str(), "已完成"))
        .count();
    let attention_count = all_rows
        .iter()
        .filter(|row| matches!(row.status.as_str(), "紧急" | "待接单" | "处理中"))
        .count();
    let query_value = query().trim().to_lowercase();
    let status_value = status_filter();
    let rows = all_rows
        .into_iter()
        .filter(|row| {
            (status_value == "全部" || row.status == status_value)
                && (query_value.is_empty()
                    || format!(
                        "{} {} {} {}",
                        row.code, row.title, row.location, row.summary
                    )
                    .to_lowercase()
                    .contains(&query_value))
        })
        .collect::<Vec<_>>();
    let parks = state.parks.read().clone();
    let selected_park = draft().park_id.parse::<u64>().ok();
    let factories = state
        .factories
        .read()
        .iter()
        .filter(|row| selected_park == Some(row.park_id))
        .cloned()
        .collect::<Vec<_>>();

    let total = rows.len();
    let page_count = total_pages(total, PAGE_SIZE);
    // 筛选收窄会让页数变少，越界时回落到最后一页，否则列表显示空白。
    let visible = rows
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();
    let status_value_signal: ReadSignal<Option<String>> =
        use_memo(move || Some(status_filter())).into();
    let draft_park_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().park_id)).into();
    let draft_factory_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().factory_id)).into();
    let draft_source_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().source)).into();
    let draft_type_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().repair_type)).into();
    let draft_priority_value: ReadSignal<Option<String>> =
        use_memo(move || Some(draft().priority)).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "报修工单" }
                    p { class: "page-subtitle", "承接租户与物业报修，按照接单、完工、验收闭环处理。" }
                }
                div { class: "page-actions",
                    Button {
                        r#type: "button",
                        onclick: move |_| {
                            draft.set(OrderDraft::fresh(state));
                            feedback.set(None);
                            form_open.set(true);
                        },
                        "新增工单"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3", aria_label: "工单状态总览",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "当前台账" }
                                strong { class: "stat-value is-mono", "{total}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已完成" }
                                strong { class: "stat-value is-mono is-ok", "{normal_count}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待处理 / 进行中" }
                                strong { class: "stat-value is-mono", "{attention_count}" }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                Card {
                    CardContent {
                        div { class: "filters",
                            div { class: "field",
                                Label { html_for: "maintenance-query", "搜索" }
                                Input {
                                    id: "maintenance-query",
                                    value: query,
                                    placeholder: "编号、园区、厂房或内容",
                                    oninput: move |event: FormEvent| {
                                        query.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "maintenance-status", "状态" }
                                Select {
                                    id: "maintenance-status",
                                    value: Some(status_value_signal),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_else(|| "全部".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    for (index , value) in ["待接单", "处理中", "待验收", "已完成", "已取消"]
                                        .into_iter()
                                        .enumerate()
                                    {
                                        SelectOption::<String> {
                                            key: "status-{value}",
                                            value: value.to_string(),
                                            index: index + 1,
                                            text_value: value.to_string(),
                                            "{value}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "报修工单台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 条" }
                }
                if visible.is_empty() {
                    p { class: "empty", "暂无符合条件的工单。可以调整筛选条件，或新增第一条工单。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "编号 / 项目" }
                                    th { "园区位置" }
                                    th { "状态" }
                                    th { "提交时间" }
                                    th { "负责人" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in visible {
                                    tr { key: "{row.id}",
                                        td {
                                            div { class: "stack-tight",
                                                strong { "{row.title}" }
                                                small { class: "hint is-mono", "{row.code}" }
                                            }
                                        }
                                        td { "{row.location}" }
                                        td {
                                            div { class: "stack-tight",
                                                Badge { variant: status_variant(&row.status), "{row.status}" }
                                                if !row.priority.is_empty() {
                                                    Badge { variant: status_variant(&row.priority), "{row.priority}" }
                                                }
                                            }
                                        }
                                        td { class: "is-mono", "{row.checked_at}" }
                                        td { "{row.operator}" }
                                        td {
                                            div { class: "table-actions",
                                                // 工单状态机：接单 → 完工 → 验收，退回和取消是分支。
                                                // 每个状态只暴露当前合法的流转动作。
                                                if row.status == "待接单" {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let id = row.id;
                                                            move |_| run_workflow(id, "accept", None, &mut feedback, &mut saving)
                                                        },
                                                        "接单"
                                                    }
                                                }
                                                if row.status == "处理中" {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let id = row.id;
                                                            move |_| {
                                                                workflow.set(Some((id, "finish".into(), "提交完工".into())));
                                                                workflow_remark.set(String::new());
                                                            }
                                                        },
                                                        "完工"
                                                    }
                                                }
                                                if row.status == "待验收" {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let id = row.id;
                                                            move |_| run_workflow(id, "verify", None, &mut feedback, &mut saving)
                                                        },
                                                        "验收"
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let id = row.id;
                                                            move |_| {
                                                                workflow.set(Some((id, "return".into(), "退回处理".into())));
                                                                workflow_remark.set(String::new());
                                                            }
                                                        },
                                                        "退回"
                                                    }
                                                }
                                                if matches!(row.status.as_str(), "待接单" | "处理中") {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        class: "is-quiet-danger",
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let id = row.id;
                                                            move |_| {
                                                                workflow.set(Some((id, "cancel".into(), "取消工单".into())));
                                                                workflow_remark.set(String::new());
                                                            }
                                                        },
                                                        "取消"
                                                    }
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let id = row.id;
                                                        move |_| {
                                                            if let Some(value) = draft_for(id, state) {
                                                                draft.set(value);
                                                                feedback.set(None);
                                                                form_open.set(true);
                                                            }
                                                        }
                                                    },
                                                    "编辑"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    class: "is-quiet-danger",
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let id = row.id;
                                                        move |_| confirm_delete.set(Some(id))
                                                    },
                                                    "删除"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Pager { page, total_pages: page_count, total_count: total }
                }
            }
        }

        if form_open() {
            Dialog {
                open: Some(true),
                on_open_change: move |open: bool| {
                    if !open && !saving() {
                        form_open.set(false);
                    }
                },
                DialogTitle {
                    if draft().id.is_some() {
                        "编辑报修工单"
                    } else {
                        "新增报修工单"
                    }
                }
                DialogDescription { "承接租户与物业报修，按照接单、完工、验收闭环处理。" }

                div { class: "stack",
                    div { class: "form-grid",
                        div { class: "field",
                            Label { html_for: "maintenance-park", "所属园区 *" }
                            Select {
                                id: "maintenance-park",
                                value: Some(draft_park_value),
                                on_value_change: move |value: Option<String>| {
                                    let mut next = draft();
                                    next.park_id = value.unwrap_or_default();
                                    // 换园区后原来的厂房不再属于这个园区，必须清空。
                                    next.factory_id.clear();
                                    draft.set(next);
                                },
                                SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                                for (index , park) in parks.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "maintenance-park-{park.park_id}",
                                        value: park.park_id.to_string(),
                                        index: index + 1,
                                        text_value: park.park_name.to_string(),
                                        "{park.park_name}"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-factory", "所属厂房" }
                            Select {
                                id: "maintenance-factory",
                                value: Some(draft_factory_value),
                                on_value_change: move |value: Option<String>| {
                                    draft.write().factory_id = value.unwrap_or_default()
                                },
                                SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择厂房".to_string(), "请选择厂房" }
                                for (index , factory) in factories.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "maintenance-factory-{factory.factory_id}",
                                        value: factory.factory_id.to_string(),
                                        index: index + 1,
                                        text_value: factory.factory_name.to_string(),
                                        "{factory.factory_name}"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-source", "报修来源 *" }
                            Select {
                                id: "maintenance-source",
                                value: Some(draft_source_value),
                                on_value_change: move |value: Option<String>| {
                                    draft.write().source = value.unwrap_or_else(|| "租户报修".into())
                                },
                                SelectOption::<String> { value: "租户报修".to_string(), index: 0usize, text_value: "租户报修".to_string(), "租户报修" }
                                SelectOption::<String> { value: "物业代报修".to_string(), index: 1usize, text_value: "物业代报修".to_string(), "物业代报修" }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-type", "报修类型 *" }
                            Select {
                                id: "maintenance-type",
                                value: Some(draft_type_value),
                                on_value_change: move |value: Option<String>| {
                                    draft.write().repair_type = value.unwrap_or_else(|| "其他".into())
                                },
                                for (index , value) in ["电路", "水暖", "设备", "消防", "电梯", "其他"]
                                    .into_iter()
                                    .enumerate()
                                {
                                    SelectOption::<String> {
                                        key: "repair-type-{value}",
                                        value: value.to_string(),
                                        index,
                                        text_value: value.to_string(),
                                        "{value}"
                                    }
                                }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-priority", "优先级 *" }
                            Select {
                                id: "maintenance-priority",
                                value: Some(draft_priority_value),
                                on_value_change: move |value: Option<String>| {
                                    draft.write().priority = value.unwrap_or_else(|| "普通".into())
                                },
                                SelectOption::<String> { value: "普通".to_string(), index: 0usize, text_value: "普通".to_string(), "普通" }
                                SelectOption::<String> { value: "紧急".to_string(), index: 1usize, text_value: "紧急".to_string(), "紧急" }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-tenant", "租户名称" }
                            Input {
                                id: "maintenance-tenant",
                                value: draft().tenant_name,
                                oninput: move |event: FormEvent| {
                                    draft.write().tenant_name = event.value()
                                },
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-tenant-phone", "联系电话" }
                            Input {
                                id: "maintenance-tenant-phone",
                                value: draft().tenant_phone,
                                inputmode: "tel",
                                oninput: move |event: FormEvent| {
                                    draft.write().tenant_phone = event.value()
                                },
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-assignee", "维修人员" }
                            Input {
                                id: "maintenance-assignee",
                                value: draft().assignee,
                                oninput: move |event: FormEvent| draft.write().assignee = event.value(),
                            }
                        }
                        div { class: "field",
                            Label { html_for: "maintenance-assignee-phone", "维修人员电话" }
                            Input {
                                id: "maintenance-assignee-phone",
                                value: draft().assignee_phone,
                                inputmode: "tel",
                                oninput: move |event: FormEvent| {
                                    draft.write().assignee_phone = event.value()
                                },
                            }
                        }
                        div { class: "field is-wide",
                            Label { html_for: "maintenance-description", "问题描述 *" }
                            Textarea {
                                id: "maintenance-description",
                                maxlength: 500,
                                rows: 3,
                                value: draft().description,
                                placeholder: "详细描述故障现象、位置和影响",
                                oninput: move |event: FormEvent| {
                                    draft.write().description = event.value()
                                },
                            }
                        }
                    }

                    if let Some(message) = feedback() {
                        p { class: "form-error", role: "alert", "{message}" }
                    }

                    div { class: "form-actions",
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            disabled: saving(),
                            onclick: move |_| form_open.set(false),
                            "取消"
                        }
                        Button {
                            r#type: "button",
                            disabled: saving(),
                            onclick: move |_| {
                                submit_draft(draft(), &mut saving, &mut feedback, &mut form_open)
                            },
                            if saving() {
                                "正在保存…"
                            } else {
                                "确认保存"
                            }
                        }
                    }
                }
            }
        }

        if let Some(id) = confirm_delete() {
            ConfirmDialog {
                title: "确认删除这条工单？",
                description: "删除后不可恢复，相关实时订阅会立即更新。",
                confirm_label: "确认删除",
                on_cancel: move |_| confirm_delete.set(None),
                on_confirm: move |_| {
                    confirm_delete.set(None);
                    delete_record(id, &mut feedback);
                },
            }
        }

        if let Some((id, action, title)) = workflow() {
            Dialog {
                open: Some(true),
                on_open_change: move |open: bool| {
                    if !open {
                        workflow.set(None);
                    }
                },
                DialogTitle { "{title}" }
                DialogDescription { "请填写本次工单处理说明，保存后状态会实时流转。" }

                div { class: "stack",
                    div { class: "field",
                        Label { html_for: "workflow-remark", "处理说明" }
                        Textarea {
                            id: "workflow-remark",
                            maxlength: 500,
                            rows: 3,
                            value: workflow_remark(),
                            placeholder: "输入处理说明",
                            oninput: move |event: FormEvent| workflow_remark.set(event.value()),
                        }
                    }
                    div { class: "form-actions",
                        Button {
                            variant: ButtonVariant::Outline,
                            r#type: "button",
                            onclick: move |_| workflow.set(None),
                            "取消"
                        }
                        Button {
                            r#type: "button",
                            onclick: move |_| {
                                let remark = workflow_remark();
                                workflow.set(None);
                                run_workflow(id, &action, optional(remark), &mut feedback, &mut saving);
                            },
                            "确认提交"
                        }
                    }
                }
            }
        }
    }
}

fn submit_draft(
    value: OrderDraft,
    saving: &mut SyncSignal<bool>,
    feedback: &mut SyncSignal<Option<String>>,
    form_open: &mut SyncSignal<bool>,
) {
    let park_id = match value.park_id.parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            feedback.set(Some("请选择所属园区".into()));
            return;
        }
    };
    let factory_id = value.factory_id.parse::<u64>().ok();
    let callback_saving = *saving;
    let callback_feedback = *feedback;
    let callback_open = *form_open;
    let callback = move |result: Result<(), String>| {
        let mut saving = callback_saving;
        let mut feedback = callback_feedback;
        let mut open = callback_open;
        saving.set(false);
        match result {
            Ok(()) => {
                feedback.set(Some("保存成功，台账正在实时更新".into()));
                open.set(false);
            }
            Err(error) => feedback.set(Some(error)),
        }
    };
    saving.set(true);
    feedback.set(None);
    let result = save_repair_order(
        value.id,
        RepairOrderInput {
            source: value.source,
            tenant_name: optional(value.tenant_name),
            tenant_phone: optional(value.tenant_phone),
            repair_type: value.repair_type,
            description: value.description,
            priority: value.priority,
            assignee: optional(value.assignee),
            assignee_phone: optional(value.assignee_phone),
            factory_id,
            park_id,
        },
        callback,
    );
    if let Err(error) = result {
        saving.set(false);
        feedback.set(Some(error));
    }
}

fn delete_record(id: u64, feedback: &mut SyncSignal<Option<String>>) {
    let callback_feedback = *feedback;
    let callback = move |result: Result<(), String>| {
        let mut feedback = callback_feedback;
        feedback.set(Some(match result {
            Ok(()) => "记录已删除".into(),
            Err(error) => error,
        }));
    };
    if let Err(error) = delete_repair_order_record(id, callback) {
        feedback.set(Some(error));
    }
}

fn run_workflow(
    id: u64,
    action: &str,
    remark: Option<String>,
    feedback: &mut SyncSignal<Option<String>>,
    saving: &mut SyncSignal<bool>,
) {
    let callback_feedback = *feedback;
    let callback_saving = *saving;
    saving.set(true);
    let result = transition_repair_order_record(id, action.into(), remark, move |result| {
        let mut feedback = callback_feedback;
        let mut saving = callback_saving;
        saving.set(false);
        feedback.set(Some(match result {
            Ok(()) => "工单状态已更新".into(),
            Err(error) => error,
        }));
    });
    if let Err(error) = result {
        saving.set(false);
        feedback.set(Some(error));
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn MaintenanceRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { MaintenanceRepairOrderPage {} }
    }

    #[test]
    fn 报修工单页面可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { MaintenanceRenderTestRoot {} });
        assert!(html.contains("报修工单"), "页面标题没有进入渲染结果");
        assert!(html.contains("报修工单台账"), "页面台账没有进入渲染结果");
    }
}
