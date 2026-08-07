//! 报销详情与审核决策弹窗。

use dioxus::prelude::*;

use super::model::{format_date, format_money, ReimbursementStatus};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        label::Label,
        textarea::Textarea,
    },
    services::audit_reimbursement_record,
    spacetime_bindings::{
        reimbursement_image_preview_type::ReimbursementImagePreview,
        reimbursement_type::Reimbursement,
    },
};

#[component]
pub(super) fn ReimbursementAuditDialog(
    row: Reimbursement,
    park_name: String,
    images: Vec<ReimbursementImagePreview>,
    can_audit: bool,
    audit_limit: Option<i64>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let status = ReimbursementStatus::from_value(row.status);
    let mut decision = use_signal(|| 1i8);
    let mut opinion = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });
    let over_limit = audit_limit.is_some_and(|limit| row.amount_cents > limit);
    let amount_label = format_money(row.amount_cents);
    let applicant_name = row.username.clone().unwrap_or_else(|| "--".into());
    let department_name = row.department.clone().unwrap_or_else(|| "--".into());
    let date_label = format_date(row.reimbursement_date);
    let image_count = images.len();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle {
                if row.status == 0 {
                    "审核报销申请"
                } else {
                    "报销审核详情"
                }
            }
            DialogDescription { "EX-{row.id:06} · {park_name}" }

            div { class: "stack",
                div { class: "panel is-tight is-plain",
                    span { class: "stat-label", "报销金额" }
                    strong { class: "stat-value is-compact is-mono", "{amount_label}" }
                    div { class: "row",
                        Badge { variant: status.badge_variant(), "{status.label()}" }
                        if over_limit {
                            Badge { variant: BadgeVariant::Destructive, "超出当前审核额度" }
                        }
                    }
                }

                dl { class: "facts",
                    div {
                        dt { "申请人" }
                        dd { "{applicant_name}" }
                    }
                    div {
                        dt { "收款人" }
                        dd { "{row.payee}" }
                    }
                    div {
                        dt { "报销日期" }
                        dd { class: "is-mono", "{date_label}" }
                    }
                    div {
                        dt { "所属园区" }
                        dd { "{park_name}" }
                    }
                    div {
                        dt { "部门" }
                        dd { "{department_name}" }
                    }
                    div { class: "is-wide",
                        dt { "报销事由" }
                        dd { "{row.purpose}" }
                    }
                    if let Some(remark) = &row.remark {
                        div { class: "is-wide",
                            dt { "申请备注" }
                            dd { "{remark}" }
                        }
                    }
                    if let Some(audit_opinion) = &row.audit_opinion {
                        div { class: "is-wide",
                            dt { "审核意见" }
                            dd { "{audit_opinion}" }
                        }
                    }
                }

                div { class: "subsection",
                    div { class: "section-header",
                        h4 { "报销凭证" }
                        Badge { variant: BadgeVariant::Outline, "{image_count} 张" }
                    }
                    if images.is_empty() {
                        p { class: "empty", "本申请未上传凭证" }
                    } else {
                        div { class: "grid-3",
                            for image in &images {
                                a {
                                    key: "proof-{image.img_id}",
                                    class: "card-media",
                                    href: "{image.img_url}",
                                    target: "_blank",
                                    rel: "noreferrer",
                                    img { src: "{image.img_url}", alt: "报销凭证", loading: "lazy" }
                                }
                            }
                        }
                    }
                }

                if row.status == 0 && can_audit {
                    form {
                        onsubmit: move |event| {
                            event.prevent_default();
                            if loading() {
                                return;
                            }
                            if over_limit {
                                error
                                    .set(
                                        Some("该金额超出当前账号审核额度，请交由更高权限角色处理".into()),
                                    );
                                return;
                            }
                            if opinion().chars().count() > 200 {
                                error.set(Some("审核意见不能超过 200 个字符".into()));
                                return;
                            }
                            let status_value = decision();
                            let opinion_value = (!opinion().trim().is_empty())
                                .then(|| opinion().trim().to_string());
                            loading.set(true);
                            error.set(None);
                            spawn(async move {
                                match audit_reimbursement_record(row.id, status_value, opinion_value)
                                    .await
                                {
                                    Ok(()) => completed.set(true),
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(message));
                                    }
                                }
                            });
                        },
                        div { class: "subsection",
                            span { class: "field-label", "审核结论" }
                            div { class: "grid-2",
                                label {
                                    class: if decision() == 1 { "tile is-selected" } else { "tile" },
                                    input {
                                        r#type: "radio",
                                        name: "decision",
                                        value: "1",
                                        checked: decision() == 1,
                                        onchange: move |_| decision.set(1),
                                    }
                                    span { class: "tile-label", "通过申请" }
                                    small { class: "hint", "同步生成财务支出记录" }
                                }
                                label {
                                    class: if decision() == 2 { "tile is-selected" } else { "tile" },
                                    input {
                                        r#type: "radio",
                                        name: "decision",
                                        value: "2",
                                        checked: decision() == 2,
                                        onchange: move |_| decision.set(2),
                                    }
                                    span { class: "tile-label", "驳回申请" }
                                    small { class: "hint", "退回申请人重新核对" }
                                }
                            }
                        }
                        div { class: "field",
                            Label { html_for: "audit-opinion", "审核意见" }
                            Textarea {
                                id: "audit-opinion",
                                value: opinion(),
                                maxlength: 200,
                                rows: 3,
                                placeholder: if decision() == 2 { "建议填写驳回原因" } else { "选填审核说明" },
                                oninput: move |event: FormEvent| opinion.set(event.value()),
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
                                // 超额时按钮保持禁用：服务端也会拒绝，前端先挡一道
                                disabled: loading() || over_limit,
                                if loading() {
                                    "正在提交…"
                                } else {
                                    "确认审核"
                                }
                            }
                        }
                    }
                } else {
                    div { class: "form-actions",
                        Button { r#type: "button", onclick: move |_| on_close.call(()), "关闭" }
                    }
                }
            }
        }
    }
}
