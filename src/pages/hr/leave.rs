//! 请假申请、修改、审批和删除页面。

use dioxus::prelude::*;

use super::{
    model::{format_datetime, leave_status, parse_datetime, today},
    navigation::HrmNavigation,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        ConfirmDialog, DateField, TimeField,
    },
    permissions::can_manage_hr,
    services::{
        audit_leave_record, create_leave_record, delete_leave_record, park_ref, update_leave_record,
    },
    spacetime_bindings::{
        leave_application_input_type::LeaveApplicationInput,
        leave_application_type::LeaveApplication, park_type::Park,
    },
    state::WorkspaceState,
};

#[component]
pub fn HrmLeavePage() -> Element {
    let state = use_context::<WorkspaceState>();
    let rows = (state.leave_applications)();
    let parks = (state.parks)();
    let admin = can_manage_hr(&(state.roles)(), &(state.permission_codes)());
    let business_user = (state.business_user)();
    let mut form_row = use_signal(|| None::<LeaveApplication>);
    let mut form_open = use_signal(|| false);
    let mut audit_row = use_signal(|| None::<LeaveApplication>);
    let mut deleting = use_signal(|| None::<LeaveApplication>);
    let mut status = use_signal(|| "all".to_string());
    let mut notice = use_signal(|| None::<String>);
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status())).into();
    let filtered = rows
        .iter()
        .filter(|row| status() == "all" || row.status.to_string() == status())
        .cloned()
        .collect::<Vec<_>>();
    let pending = rows.iter().filter(|row| row.status == 0).count();
    let approved = rows.iter().filter(|row| row.status == 1).count();
    let rejected = rows.iter().filter(|row| row.status == 2).count();

    rsx! {
        main { class: "page",
            HrmNavigation { active: String::from("leave") }

            header { class: "page-header",
                div { class: "page-title",
                    h1 { "请假申请" }
                    p { class: "page-subtitle", "员工提交申请，人事审核后实时回写审批结果。" }
                }
                div { class: "page-actions",
                    Button {
                        onclick: move |_| {
                            form_row.set(None);
                            form_open.set(true);
                        },
                        "新增请假"
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "申请总数" }
                                strong { class: "stat-value is-mono", "{rows.len()}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待审批" }
                                strong { class: "stat-value is-mono", "{pending}" }
                                span { class: "stat-caption", "需人事处理" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已批准" }
                                strong { class: "stat-value is-mono is-ok", "{approved}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已驳回" }
                                strong { class: "stat-value is-mono", "{rejected}" }
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
                                Label { html_for: "leave-status", "审批状态" }
                                Select {
                                    id: "leave-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status.set(value.unwrap_or_else(|| "all".into()));
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "待审批".to_string(), "待审批" }
                                    SelectOption::<String> { value: "1".to_string(), index: 2usize, text_value: "已批准".to_string(), "已批准" }
                                    SelectOption::<String> { value: "2".to_string(), index: 3usize, text_value: "已驳回".to_string(), "已驳回" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 {
                        if admin {
                            "请假审批台账"
                        } else {
                            "我的请假记录"
                        }
                    }
                    Badge { variant: BadgeVariant::Secondary, "{filtered.len()} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "申请人" }
                                th { "类型" }
                                th { "状态" }
                                th { "时间范围" }
                                th { "园区" }
                                th { "事由" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if filtered.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "7", "暂无请假申请" }
                                }
                            }
                            for row in filtered {
                                {
                                    let (status_label, status_variant) = leave_status(row.status);
                                    let applicant = row
                                        .applicant_name
                                        .clone()
                                        .or(row.username.clone())
                                        .unwrap_or_else(|| "--".into());
                                    let park = row.park_name.clone().unwrap_or_else(|| "未分配园区".into());
                                    let period = format!(
                                        "{} — {}",
                                        format_datetime(Some(row.start_date)),
                                        format_datetime(Some(row.end_date)),
                                    );
                                    let edit = row.clone();
                                    let audit = row.clone();
                                    let remove = row.clone();
                                    let own = business_user
                                        .as_ref()
                                        .is_some_and(|user| row.user_id == Some(user.id));
                                    rsx! {
                                        tr { key: "leave-{row.id}",
                                            td {
                                                strong { "{applicant}" }
                                            }
                                            td { "{row.leave_type}" }
                                            td {
                                                Badge { variant: status_variant, "{status_label}" }
                                            }
                                            td { class: "is-mono", "{period}" }
                                            td { "{park}" }
                                            td { class: "is-wrap hint", "{row.reason}" }
                                            td {
                                                div { class: "table-actions",
                                                    Button {
                                                        variant: if admin && row.status == 0 { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: move |_| audit_row.set(Some(audit.clone())),
                                                        if admin && row.status == 0 {
                                                            "审批"
                                                        } else {
                                                            "查看"
                                                        }
                                                    }
                                                    if own && row.status == 0 {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| {
                                                                form_row.set(Some(edit.clone()));
                                                                form_open.set(true);
                                                            },
                                                            "修改"
                                                        }
                                                    }
                                                    if admin || (own && row.status == 0) {
                                                        Button {
                                                            variant: ButtonVariant::Outline,
                                                            class: "is-quiet-danger",
                                                            size: ButtonSize::Sm,
                                                            r#type: "button",
                                                            onclick: move |_| deleting.set(Some(remove.clone())),
                                                            "删除"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if form_open() {
            LeaveFormDialog {
                row: form_row(),
                parks: parks.clone(),
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("请假申请已保存".into()));
                },
            }
        }
        if let Some(row) = audit_row() {
            LeaveAuditDialog {
                row,
                can_audit: admin,
                on_close: move |_| audit_row.set(None),
                on_saved: move |_| {
                    audit_row.set(None);
                    notice.set(Some("审批结果已保存".into()));
                },
            }
        }
        if let Some(row) = deleting() {
            LeaveDeleteDialog {
                row,
                on_close: move |_| deleting.set(None),
                on_deleted: move |_| {
                    deleting.set(None);
                    notice.set(Some("请假记录已删除".into()));
                },
            }
        }
    }
}

#[component]
fn LeaveFormDialog(
    row: Option<LeaveApplication>,
    parks: Vec<Park>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let id = row.as_ref().map(|row| row.id);
    let current_date = today();
    let mut leave_type = use_signal(|| {
        row.as_ref()
            .map(|row| row.leave_type.clone())
            .unwrap_or_else(|| "事假".into())
    });
    let mut start = use_signal(|| {
        row.as_ref()
            .map(|row| format_datetime(Some(row.start_date)).replace(' ', "T"))
            .unwrap_or_else(|| format!("{current_date}T09:00"))
    });
    let mut end = use_signal(|| {
        row.as_ref()
            .map(|row| format_datetime(Some(row.end_date)).replace(' ', "T"))
            .unwrap_or_else(|| format!("{current_date}T18:00"))
    });
    let mut park_id = use_signal(|| {
        row.as_ref()
            .and_then(|row| park_ref(row.park_id))
            .map(|value| value.to_string())
            .or_else(|| parks.first().map(|park| park.park_id.to_string()))
            .unwrap_or_default()
    });
    let mut reason = use_signal(|| {
        row.as_ref()
            .map(|row| row.reason.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut done = use_signal(|| false);
    // 表单内部仍以 `YYYY-MM-DDTHH:MM` 保存，拆出日期和时间两半供各自的控件使用。
    let start_date = use_memo(move || start().split('T').next().unwrap_or_default().to_string());
    let start_time = use_memo(move || start().split('T').nth(1).unwrap_or_default().to_string());
    let end_date = use_memo(move || end().split('T').next().unwrap_or_default().to_string());
    let end_time = use_memo(move || end().split('T').nth(1).unwrap_or_default().to_string());
    let leave_type_value: ReadSignal<Option<String>> = use_memo(move || Some(leave_type())).into();
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    use_effect(move || {
        if done() {
            on_saved.call(());
        }
    });

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle {
                if id.is_some() {
                    "修改请假"
                } else {
                    "新增请假"
                }
            }
            DialogDescription { "提交后由人事审批，审批前可以自行修改。" }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    let start_date = match parse_datetime(&start()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    let end_date = match parse_datetime(&end()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    let Some(selected_park) = park_id().parse().ok() else {
                        error.set(Some("请选择园区".into()));
                        return;
                    };
                    if reason().trim().is_empty() {
                        error.set(Some("请输入请假事由".into()));
                        return;
                    }
                    let input = LeaveApplicationInput {
                        start_date,
                        end_date,
                        leave_type: leave_type(),
                        reason: reason().trim().into(),
                        park_id: selected_park,
                    };
                    loading.set(true);
                    spawn(async move {
                        let result = if let Some(id) = id {
                            update_leave_record(id, input).await
                        } else {
                            create_leave_record(input).await
                        };
                        match result {
                            Ok(()) => done.set(true),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "leave-type", "请假类型 *" }
                        Select {
                            id: "leave-type",
                            value: Some(leave_type_value),
                            on_value_change: move |value: Option<String>| {
                                leave_type.set(value.unwrap_or_else(|| "事假".into()));
                            },
                            for (index , value) in ["事假", "病假", "年假", "婚假", "产假", "调休", "其他"]
                                .into_iter()
                                .enumerate()
                            {
                                SelectOption::<String> {
                                    key: "leave-type-{value}",
                                    value: value.to_string(),
                                    index,
                                    text_value: value.to_string(), "{value}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "leave-park", "所属园区 *" }
                        Select {
                            id: "leave-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "leave-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(), "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "开始时间 *" }
                        // 日期和时间分开选：原生 datetime-local 会拉起操作系统
                        // 自带的选择器，配色和语言都不受控。
                        div { class: "row",
                            DateField {
                                value: start_date(),
                                on_change: move |value: String| start.set(format!("{value}T{}", start_time())),
                            }
                            TimeField {
                                value: start_time(),
                                on_change: move |value: String| start.set(format!("{}T{value}", start_date())),
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "结束时间 *" }
                        // 日期和时间分开选：原生 datetime-local 会拉起操作系统
                        // 自带的选择器，配色和语言都不受控。
                        div { class: "row",
                            DateField {
                                value: end_date(),
                                on_change: move |value: String| end.set(format!("{value}T{}", end_time())),
                            }
                            TimeField {
                                value: end_time(),
                                on_change: move |value: String| end.set(format!("{}T{value}", end_date())),
                            }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "leave-reason", "请假事由 *" }
                        Textarea {
                            id: "leave-reason",
                            value: reason(),
                            maxlength: 191,
                            rows: 3,
                            oninput: move |event: FormEvent| reason.set(event.value()),
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| on_close.call(()),
                        "取消"
                    }
                    Button {
                        r#type: "submit",
                        disabled: loading(),
                        if loading() {
                            "保存中…"
                        } else {
                            "确认提交"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LeaveAuditDialog(
    row: LeaveApplication,
    can_audit: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let mut decision = use_signal(|| 1i8);
    let mut reply = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut done = use_signal(|| false);
    use_effect(move || {
        if done() {
            on_saved.call(());
        }
    });
    let period = format!(
        "{} — {}",
        format_datetime(Some(row.start_date)),
        format_datetime(Some(row.end_date))
    );
    let (status_label, status_variant) = leave_status(row.status);
    let applicant = row.applicant_name.clone().unwrap_or_else(|| "--".into());

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "请假审批详情" }
            DialogDescription { "{applicant} · {row.leave_type}" }

            div { class: "stack",
                div { class: "row",
                    Badge { variant: status_variant, "{status_label}" }
                    span { class: "hint is-mono", "{period}" }
                }
                dl { class: "facts",
                    div { class: "is-wide",
                        dt { "申请事由" }
                        dd { "{row.reason}" }
                    }
                }

                if row.status == 0 && can_audit {
                    form {
                        onsubmit: move |event| {
                            event.prevent_default();
                            loading.set(true);
                            let text = (!reply().trim().is_empty()).then(|| reply().trim().into());
                            spawn(async move {
                                match audit_leave_record(row.id, decision(), text).await {
                                    Ok(()) => done.set(true),
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(message));
                                    }
                                }
                            });
                        },
                        div { class: "form-grid",
                            div { class: "field is-wide",
                                span { class: "field-label", "审批结论" }
                                div { class: "row",
                                    label { class: "row",
                                        input {
                                            r#type: "radio",
                                            name: "leave-decision",
                                            checked: decision() == 1,
                                            onchange: move |_| decision.set(1),
                                        }
                                        span { "批准" }
                                    }
                                    label { class: "row",
                                        input {
                                            r#type: "radio",
                                            name: "leave-decision",
                                            checked: decision() == 2,
                                            onchange: move |_| decision.set(2),
                                        }
                                        span { "驳回" }
                                    }
                                }
                            }
                            div { class: "field is-wide",
                                Label { html_for: "leave-reply", "审批回复" }
                                Textarea {
                                    id: "leave-reply",
                                    value: reply(),
                                    rows: 3,
                                    placeholder: "可选，会一并回写给申请人",
                                    oninput: move |event: FormEvent| reply.set(event.value()),
                                }
                            }
                        }
                        if let Some(message) = error() {
                            p { class: "form-error", role: "alert", "{message}" }
                        }
                        div { class: "form-actions",
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                disabled: loading(),
                                onclick: move |_| on_close.call(()),
                                "取消"
                            }
                            Button { r#type: "submit", disabled: loading(), "确认审批" }
                        }
                    }
                } else {
                    if let Some(reply) = &row.reply {
                        p { class: "notice", "审批回复：{reply}" }
                    }
                    div { class: "form-actions",
                        Button { onclick: move |_| on_close.call(()), "关闭" }
                    }
                }
            }
        }
    }
}

#[component]
fn LeaveDeleteDialog(
    row: LeaveApplication,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut done = use_signal(|| false);
    let leave_id = row.id;
    let leave_type = row.leave_type.clone();

    use_effect(move || {
        if done() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: "确认删除请假",
            description: format!("将删除“{leave_type}”申请，此操作不可撤销。"),
            confirm_label: "确认删除",
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                spawn(async move {
                    match delete_leave_record(leave_id).await {
                        Ok(()) => done.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(message));
                        }
                    }
                });
            },
        }
    }
}
