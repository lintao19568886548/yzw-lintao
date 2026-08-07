//! 报销申请台账和审核队列主页面。

use std::collections::HashMap;

use dioxus::prelude::*;

use super::{
    audit_dialog::ReimbursementAuditDialog,
    form::ReimbursementFormDialog,
    model::{
        audit_limit, audit_priority, can_audit, format_date, format_money, priority_reason,
        today_timestamp, waiting_days, AuditPriority, ReimbursementStatus,
    },
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, ConfirmDialog, Pager,
    },
    services::{
        delete_business_images_from_r2, delete_reimbursement_record, park_ref, StoredR2Image,
    },
    spacetime_bindings::{
        reimbursement_image_preview_type::ReimbursementImagePreview,
        reimbursement_type::Reimbursement,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageMode {
    Application,
    Audit,
}

#[component]
pub fn ReimbursementApplicationPage() -> Element {
    rsx! { ReimbursementOverview { mode: PageMode::Application } }
}

#[component]
pub fn ReimbursementAuditPage() -> Element {
    rsx! { ReimbursementOverview { mode: PageMode::Audit } }
}

fn images_for(id: u64, images: &[ReimbursementImagePreview]) -> Vec<ReimbursementImagePreview> {
    images
        .iter()
        .filter(|image| image.reimbursement_id == id)
        .cloned()
        .collect()
}

#[component]
fn ReimbursementOverview(mode: PageMode) -> Element {
    let state = use_context::<WorkspaceState>();
    let all_rows = (state.reimbursements)();
    let image_previews = (state.reimbursement_image_previews)();
    let parks = (state.parks)();
    let roles = (state.roles)();
    let current_user = state.current_user.read().clone();
    let auditor = can_audit(&roles);
    let limit = audit_limit(&roles);
    let current_user_id = current_user.as_ref().map(|user| user.id);
    let applicant = current_user
        .as_ref()
        .map(|user| user.real_name.clone())
        .unwrap_or_else(|| "当前用户".into());
    let park_map = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<HashMap<_, _>>();
    let today = today_timestamp();

    let mut keyword = use_signal(String::new);
    let mut park_filter = use_signal(String::new);
    let mut status_filter = use_signal(|| {
        if mode == PageMode::Audit {
            "0".to_string()
        } else {
            "all".to_string()
        }
    });
    let mut page = use_signal(|| 1usize);
    let mut form_open = use_signal(|| false);
    let mut detail_row = use_signal(|| None::<Reimbursement>);
    let mut delete_row = use_signal(|| None::<Reimbursement>);
    let mut notice = use_signal(|| None::<String>);

    if mode == PageMode::Audit && !auditor {
        return rsx! {
            main { class: "page",
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { "暂无报销审核权限" }
                            p { class: "page-subtitle",
                                "当前角色仍可在「报销管理」中提交并查看自己的申请。审核权限由角色 reimbursement_auth 和金额额度共同控制。"
                            }
                            div { class: "card-cta",
                                Link { to: crate::router::Route::ReimbursementApplicationPage {},
                                    Button { variant: ButtonVariant::Outline, "返回报销管理" }
                                }
                            }
                        }
                    }
                }
            }
        };
    }

    let base_rows = all_rows
        .iter()
        .filter(|row| mode == PageMode::Audit || row.user_id == current_user_id)
        .cloned()
        .collect::<Vec<_>>();
    let pending_count = base_rows.iter().filter(|row| row.status == 0).count();
    let approved_count = base_rows.iter().filter(|row| row.status == 1).count();
    let rejected_count = base_rows.iter().filter(|row| row.status == 2).count();
    let pending_amount = base_rows
        .iter()
        .filter(|row| row.status == 0)
        .map(|row| row.amount_cents)
        .sum::<i64>();
    let urgent_count = base_rows
        .iter()
        .filter(|row| audit_priority(row, today) == AuditPriority::Urgent)
        .count();

    let normalized = keyword().trim().to_lowercase();
    let selected_park = park_filter().parse::<u64>().ok();
    let selected_status = status_filter();
    let mut filtered = base_rows
        .into_iter()
        .filter(|row| {
            (normalized.is_empty()
                || row.purpose.to_lowercase().contains(&normalized)
                || row.payee.to_lowercase().contains(&normalized)
                || row
                    .username
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&normalized))
                && selected_park.is_none_or(|park_id| row.park_id == park_id)
                && (selected_status == "all" || row.status.to_string() == selected_status)
        })
        .collect::<Vec<_>>();
    if mode == PageMode::Audit && selected_status == "0" {
        filtered.sort_by_key(|row| {
            (
                match audit_priority(row, today) {
                    AuditPriority::Urgent => 0,
                    AuditPriority::Warning => 1,
                    _ => 2,
                },
                row.created_at.to_micros_since_unix_epoch(),
                std::cmp::Reverse(row.amount_cents),
            )
        });
    } else {
        filtered.sort_by_key(|row| std::cmp::Reverse(row.id));
    }
    let page_count = total_pages(filtered.len(), PAGE_SIZE);
    // 筛选收窄会让页数变少，越界时回落到最后一页，否则列表显示空白。
    let current_page = page().clamp(1, page_count);
    let page_rows = filtered
        .iter()
        .skip((current_page - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect::<Vec<_>>();
    let title = if mode == PageMode::Audit {
        "报销审核"
    } else {
        "报销管理"
    };
    let authority_label = limit.map(format_money).unwrap_or_else(|| "不限金额".into());
    let pending_amount_label = format_money(pending_amount);
    let filtered_count = filtered.len();

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_filter())).into();
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status_filter())).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "{title}" }
                    p { class: "page-subtitle",
                        if mode == PageMode::Audit {
                            "按园区权限和金额额度处理报销，审核通过后自动同步财务支出。"
                        } else {
                            "提交报销凭证，跟踪审核状态，并保留每笔费用的完整申请记录。"
                        }
                    }
                }
                div { class: "page-actions",
                    if mode == PageMode::Application {
                        Button { onclick: move |_| form_open.set(true), "新增报销" }
                    } else {
                        Badge { variant: BadgeVariant::Outline, "审核额度 {authority_label}" }
                    }
                }
            }

            if let Some(message) = notice() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-auto",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待审核" }
                                strong { class: "stat-value is-mono", "{pending_count}" }
                                span { class: "stat-caption", "待审金额 {pending_amount_label}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "紧急队列" }
                                strong { class: "stat-value is-mono", "{urgent_count}" }
                                span { class: "stat-caption", "等待过久或金额较大" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已通过" }
                                strong { class: "stat-value is-mono is-ok", "{approved_count}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "已驳回" }
                                strong { class: "stat-value is-mono", "{rejected_count}" }
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
                                Label { html_for: "expense-keyword", "事由 / 收款人 / 申请人" }
                                Input {
                                    id: "expense-keyword",
                                    value: keyword,
                                    placeholder: "输入关键词",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "expense-park", "所属园区" }
                                Select {
                                    id: "expense-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_filter.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , park) in parks.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "expense-park-{park.park_id}",
                                            value: park.park_id.to_string(),
                                            index: index + 1,
                                            text_value: park.park_name.to_string(),
                                            "{park.park_name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "expense-status", "审核状态" }
                                Select {
                                    id: "expense-status",
                                    value: Some(status_value),
                                    on_value_change: move |value: Option<String>| {
                                        status_filter.set(value.unwrap_or_else(|| "all".into()));
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: "all".to_string(), index: 0usize, text_value: "全部状态".to_string(), "全部状态" }
                                    SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "待审核".to_string(), "待审核" }
                                    SelectOption::<String> { value: "1".to_string(), index: 2usize, text_value: "已通过".to_string(), "已通过" }
                                    SelectOption::<String> { value: "2".to_string(), index: 3usize, text_value: "已驳回".to_string(), "已驳回" }
                                }
                            }
                            div { class: "field",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    r#type: "button",
                                    onclick: move |_| {
                                        keyword.set(String::new());
                                        park_filter.set(String::new());
                                        status_filter
                                            .set(if mode == PageMode::Audit { "0".into() } else { "all".into() });
                                        page.set(1);
                                    },
                                    "重置筛选"
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 {
                        if mode == PageMode::Audit {
                            "审核队列"
                        } else {
                            "我的报销记录"
                        }
                    }
                    Badge { variant: BadgeVariant::Secondary, "{filtered_count} 条" }
                }
                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "优先级" }
                                th { "申请信息" }
                                th { "园区 / 部门" }
                                th { "金额" }
                                th { "日期" }
                                th { "凭证" }
                                th { "状态" }
                                th { "操作" }
                            }
                        }
                        tbody {
                            if page_rows.is_empty() {
                                tr {
                                    td { class: "table-empty", colspan: "8", "暂无符合条件的报销记录" }
                                }
                            }
                            for row in &page_rows {
                                {
                                    let status = ReimbursementStatus::from_value(row.status);
                                    let priority = audit_priority(row, today);
                                    let reason = priority_reason(row, today);
                                    let park_name = park_ref(row.park_id).and_then(|id| park_map.get(&id).cloned())
                                        .unwrap_or_else(|| "未分配园区".into());
                                    let department = row
                                        .department
                                        .clone()
                                        .unwrap_or_else(|| "未填写部门".into());
                                    let applicant_payee = format!(
                                        "{} → {}",
                                        row.username.clone().unwrap_or_else(|| "--".into()),
                                        row.payee,
                                    );
                                    let wait_days = waiting_days(row, today);
                                    let proof_count = images_for(row.id, &image_previews).len();
                                    let detail = row.clone();
                                    let open_row = row.clone();
                                    let remove = row.clone();
                                    rsx! {
                                        // 整行打开详情/审核：凭证图片和审批意见都在弹窗里
                                        tr {
                                            key: "expense-{row.id}",
                                            class: "is-clickable",
                                            onclick: move |_| detail_row.set(Some(open_row.clone())),
                                            td {
                                                div { class: "stack-tight",
                                                    Badge { variant: priority.badge_variant(), "{priority.label()}" }
                                                    small { class: "hint", "{reason}" }
                                                }
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    strong { "{row.purpose}" }
                                                    small { class: "hint", "{applicant_payee}" }
                                                }
                                            }
                                            td {
                                                div { class: "stack-tight",
                                                    span { "{park_name}" }
                                                    small { class: "hint", "{department}" }
                                                }
                                            }
                                            td { class: "is-mono", "{format_money(row.amount_cents)}" }
                                            td {
                                                div { class: "stack-tight",
                                                    span { class: "is-mono", "{format_date(row.reimbursement_date)}" }
                                                    if row.status == 0 {
                                                        small { class: "hint", "等待 {wait_days} 天" }
                                                    }
                                                }
                                            }
                                            td { class: "is-mono", "{proof_count} 张" }
                                            td {
                                                Badge { variant: status.badge_variant(), "{status.label()}" }
                                            }
                                            td {
                                                div {
                                                    class: "table-actions",
                                                    onclick: move |event| event.stop_propagation(),
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| detail_row.set(Some(detail.clone())),
                                                        if mode == PageMode::Audit && row.status == 0 {
                                                            "审核"
                                                        } else {
                                                            "查看"
                                                        }
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        class: "is-quiet-danger",
                                                        size: ButtonSize::Sm,
                                                        onclick: move |_| delete_row.set(Some(remove.clone())),
                                                        if mode == PageMode::Application {
                                                            "撤销"
                                                        } else {
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
                Pager { page, total_pages: page_count, total_count: filtered_count }
            }
        }

        if form_open() {
            ReimbursementFormDialog {
                parks: parks.clone(),
                applicant: applicant.clone(),
                on_close: move |_| form_open.set(false),
                on_saved: move |_| {
                    form_open.set(false);
                    notice.set(Some("报销申请已进入审核队列".into()));
                },
            }
        }
        if let Some(row) = detail_row() {
            {
                let park_name = park_ref(row.park_id).and_then(|id| park_map.get(&id).cloned())
                    .unwrap_or_else(|| "未分配园区".into());
                let row_images = images_for(row.id, &image_previews);
                rsx! {
                    ReimbursementAuditDialog {
                        row,
                        park_name,
                        images: row_images,
                        can_audit: auditor && mode == PageMode::Audit,
                        audit_limit: limit,
                        on_close: move |_| detail_row.set(None),
                        on_saved: move |_| {
                            detail_row.set(None);
                            notice.set(Some("审核结果已保存".into()));
                        },
                    }
                }
            }
        }
        if let Some(row) = delete_row() {
            {
                let cleanup = images_for(row.id, &image_previews)
                    .into_iter()
                    .map(|image| StoredR2Image {
                        img_id: image.img_id,
                        public_url: image.img_url,
                    })
                    .collect::<Vec<_>>();
                rsx! {
                    ReimbursementDeleteDialog {
                        row,
                        images: cleanup,
                        application_mode: mode == PageMode::Application,
                        on_close: move |_| delete_row.set(None),
                        on_deleted: move |_| {
                            delete_row.set(None);
                            notice
                                .set(
                                    Some(
                                        if mode == PageMode::Application {
                                            "报销申请已撤销".into()
                                        } else {
                                            "报销记录已删除".into()
                                        },
                                    ),
                                );
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ReimbursementDeleteDialog(
    row: Reimbursement,
    images: Vec<StoredR2Image>,
    application_mode: bool,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    // 数据库删除成功但 R2 清理失败时，重试只需要重跑清理那一步。
    let mut database_deleted = use_signal(|| false);
    let mut completed = use_signal(|| false);
    let row_id = row.id;
    let purpose = row.purpose.clone();
    let image_count = images.len();
    let action_label = if application_mode { "撤销" } else { "删除" };

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: if application_mode { "确认撤销申请" } else { "确认删除报销" },
            description: format!(
                "将{action_label}“{purpose}”报销记录，并解除 {image_count} 张凭证关系；审核通过产生的财务记录也会归档。此操作不可撤销。",
            ),
            confirm_label: if database_deleted() {
                "重试清理".to_string()
            } else {
                format!("确认{action_label}")
            },
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                let cleanup = images.clone();
                spawn(async move {
                    if !database_deleted() {
                        match delete_reimbursement_record(row_id).await {
                            Ok(()) => database_deleted.set(true),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                                return;
                            }
                        }
                    }
                    match delete_business_images_from_r2(cleanup).await {
                        Ok(_) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(format!("报销记录已删除，但{message}；请重试清理")));
                        }
                    }
                });
            },
        }
    }
}
