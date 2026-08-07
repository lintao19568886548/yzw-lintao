//! 催收短信候选预览、选择和发送结果弹窗。

use std::collections::BTreeMap;

use dioxus::prelude::*;

use super::model::{format_money, remaining_cents};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        DateField,
    },
    services::send_collection_sms,
    spacetime_bindings::{
        amount_bill_type::AmountBill, park_type::Park, rental_tenant_type::RentalTenant,
    },
};

fn preview_message(bill: &AmountBill, tenant_name: &str, collection_type: &str) -> String {
    let tail = match collection_type {
        "overdue_10" => "已逾期，请尽快完成核对和缴费",
        "final_30" => "已长期未结，请及时联系园区财务处理",
        _ => "尚未结清，请在约定日期前完成核对和缴费",
    };
    format!(
        "{tenant_name}您好，{}待结金额{}，{tail}。如已处理请忽略。",
        bill.project_name,
        format_money(remaining_cents(bill))
    )
}

#[component]
pub(super) fn BillCollectionDialog(
    bills: Vec<AmountBill>,
    tenants: Vec<RentalTenant>,
    parks: Vec<Park>,
    on_close: EventHandler<()>,
) -> Element {
    let tenant_map = tenants
        .iter()
        .map(|tenant| (tenant.rental_tenant_id, tenant.clone()))
        .collect::<BTreeMap<_, _>>();
    let park_map = parks
        .iter()
        .map(|park| (park.park_id, park.park_name.clone()))
        .collect::<BTreeMap<_, _>>();
    let candidate_ids = bills
        .iter()
        .filter(|bill| remaining_cents(bill) > 0)
        .filter(|bill| {
            tenant_map.get(&bill.tenant_id).is_some_and(|tenant| {
                let phone = tenant
                    .phone_number
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>();
                phone.len() == 11 && phone.starts_with('1')
            })
        })
        .map(|bill| bill.bill_id)
        .collect::<Vec<_>>();
    let mut selected_ids = use_signal(|| candidate_ids);
    let mut collection_type = use_signal(|| "payment_reminder".to_string());
    let mut due_date = use_signal(String::new);
    let mut overdue_days = use_signal(|| 10u32);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result_text = use_signal(|| None::<String>);

    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(collection_type())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "催收短信补发" }
            DialogDescription { "只发送选中的未结账单；发送结果由 Module 写入审计日志。" }

            div { class: "stack",
                div { class: "filters",
                    div { class: "field",
                        Label { html_for: "collection-type", "催收类型" }
                        Select {
                            id: "collection-type",
                            value: Some(type_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| {
                                let value = value.unwrap_or_else(|| "payment_reminder".into());
                                // 长期未结默认按 30 天算，其余按 10 天。
                                overdue_days.set(if value == "final_30" { 30 } else { 10 });
                                collection_type.set(value);
                            },
                            SelectOption::<String> { value: "payment_reminder".to_string(), index: 0usize, text_value: "缴费提醒".to_string(), "缴费提醒" }
                            SelectOption::<String> { value: "overdue_10".to_string(), index: 1usize, text_value: "逾期提醒".to_string(), "逾期提醒" }
                            SelectOption::<String> { value: "final_30".to_string(), index: 2usize, text_value: "长期未结提醒".to_string(), "长期未结提醒" }
                        }
                    }
                    if collection_type() == "payment_reminder" {
                        div { class: "field",
                            span { class: "field-label", "缴费截止日期" }
                            DateField {
                                value: due_date(),
                                disabled: loading(),
                                on_change: move |value: String| due_date.set(value),
                            }
                        }
                    } else {
                        div { class: "field",
                            Label { html_for: "collection-overdue", "逾期天数" }
                            Input {
                                id: "collection-overdue",
                                r#type: "number",
                                min: "1",
                                value: overdue_days().to_string(),
                                disabled: loading(),
                                oninput: move |event: FormEvent| {
                                    overdue_days.set(event.value().parse().unwrap_or(10))
                                },
                            }
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "待发送" }
                        span { class: "field-static is-mono", "{selected_ids().len()} 份" }
                    }
                }

                if bills.iter().all(|bill| remaining_cents(bill) <= 0) {
                    p { class: "empty", "当前筛选账单均已结清" }
                } else {
                    div { class: "list",
                        for bill in bills.iter().filter(|bill| remaining_cents(bill) > 0) {
                            {
                                let tenant = tenant_map.get(&bill.tenant_id);
                                let tenant_name = tenant
                                    .map(|row| row.tenant_name.as_str())
                                    .or(bill.tenant_name.as_deref())
                                    .unwrap_or("未关联租户");
                                let phone = tenant.map(|row| row.phone_number.as_str()).unwrap_or("");
                                let normalized_phone = phone
                                    .chars()
                                    .filter(char::is_ascii_digit)
                                    .collect::<String>();
                                // 手机号不合法就发不出去，条目置灰且不可勾选
                                let can_send = normalized_phone.len() == 11
                                    && normalized_phone.starts_with('1');
                                let checked = selected_ids().contains(&bill.bill_id);
                                let bill_id = bill.bill_id;
                                let park_name = park_map
                                    .get(&bill.park_id)
                                    .map(String::as_str)
                                    .unwrap_or("园区已移除");
                                let project_name = bill.project_name.clone();
                                let message_preview = preview_message(
                                    bill,
                                    tenant_name,
                                    &collection_type(),
                                );
                                let remaining = format_money(remaining_cents(bill));
                                let masked_phone = if can_send {
                                    format!("{}****{}", &normalized_phone[..3], &normalized_phone[7..])
                                } else {
                                    "手机号无效".into()
                                };
                                rsx! {
                                    label {
                                        key: "collection-{bill_id}",
                                        class: if can_send { "list-item" } else { "list-item is-muted" },
                                        input {
                                            r#type: "checkbox",
                                            checked,
                                            disabled: loading() || !can_send,
                                            onchange: move |_| {
                                                selected_ids
                                                    .with_mut(|ids| {
                                                        if ids.contains(&bill_id) {
                                                            ids.retain(|id| *id != bill_id);
                                                        } else {
                                                            ids.push(bill_id);
                                                        }
                                                    });
                                            },
                                        }
                                        div { class: "list-item-copy",
                                            strong { "{tenant_name}" }
                                            small { "{park_name} · {project_name}" }
                                            small { "{message_preview}" }
                                        }
                                        div { class: "stack-tight",
                                            strong { class: "is-mono", "{remaining}" }
                                            small { class: "hint is-mono", "{masked_phone}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(message) = result_text() {
                    p { class: "notice", role: "status", "{message}" }
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
                        "关闭"
                    }
                    Button {
                        r#type: "button",
                        disabled: loading() || selected_ids().is_empty(),
                        onclick: move |_| {
                            if selected_ids().is_empty() {
                                return;
                            }
                            loading.set(true);
                            error.set(None);
                            result_text.set(None);
                            let ids = selected_ids();
                            spawn(async move {
                                match send_collection_sms(
                                        ids,
                                        collection_type(),
                                        due_date(),
                                        overdue_days(),
                                    )
                                    .await
                                {
                                    Ok(batch) => {
                                        let success = batch.results.iter().filter(|item| item.success).count();
                                        let failed = batch.results.len().saturating_sub(success);
                                        result_text
                                            .set(Some(format!("发送完成：成功 {success} 条，失败 {failed} 条")));
                                        if failed > 0 {
                                            error
                                                .set(
                                                    Some(
                                                        batch
                                                            .results
                                                            .iter()
                                                            .filter(|item| !item.success)
                                                            .map(|item| format!("#{} {}", item.bill_id, item.message))
                                                            .collect::<Vec<_>>()
                                                            .join("；"),
                                                    ),
                                                );
                                        }
                                    }
                                    Err(message) => error.set(Some(message)),
                                }
                                loading.set(false);
                            });
                        },
                        if loading() {
                            "正在发送…"
                        } else {
                            "确认发送"
                        }
                    }
                }
            }
        }
    }
}
