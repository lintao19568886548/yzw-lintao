//! 账单打印预览。

use dioxus::prelude::*;

use super::{
    model::{collection_status, format_date, format_money, remaining_cents},
    worksheet::BankAccountDraft,
};
use crate::{
    components::{
        badge::Badge,
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
    },
    spacetime_bindings::amount_bill_type::AmountBill,
};

#[cfg(target_arch = "wasm32")]
fn print_window() {
    if let Some(window) = web_sys::window() {
        let _ = window.print();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn print_window() {}

#[component]
pub(super) fn BillPrintDialog(
    bill: AmountBill,
    park_name: String,
    on_close: EventHandler<()>,
) -> Element {
    let status = collection_status(&bill);
    // 库里存的是 JSON，直接打印会露出 {"bank":"…","name":"…"} 这样的原始字符串。
    let public_account = BankAccountDraft::from_stored(bill.public_bank_account.as_deref())
        .display_line()
        .unwrap_or_else(|| "--".into());
    let private_account = BankAccountDraft::from_stored(bill.private_bank_account.as_deref())
        .display_line()
        .unwrap_or_else(|| "--".into());
    let line_items = [
        ("厂房租金", bill.factory_rent_cents),
        ("基本管理费", bill.management_fee_cents),
        ("垃圾管理费", bill.garbage_fee_cents),
        ("服务费", bill.service_fee_cents),
        ("电费", bill.ele_fee_cents),
        ("水费", bill.water_fee_cents),
        ("其他应收", bill.receive_fee_cents),
        ("开票税金", bill.invoice_tax_cents),
        ("滞纳金", bill.penalty_fee_cents.unwrap_or_default()),
    ];
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "园区费用账单" }
            DialogDescription { "YZ-{bill.bill_id:08}" }

            div { class: "stack",
                dl { class: "facts",
                    div {
                        dt { "园区" }
                        dd { "{park_name}" }
                    }
                    div {
                        dt { "租赁客户" }
                        dd { {bill.tenant_name.as_deref().unwrap_or("--")} }
                    }
                    div {
                        dt { "账单项目" }
                        dd { "{bill.project_name}" }
                    }
                    div {
                        dt { "收款状态" }
                        dd {
                            Badge { variant: status.badge_variant(), "{status.label()}" }
                        }
                    }
                }

                div { class: "table-shell",
                    table { class: "table",
                        thead {
                            tr {
                                th { "费用项目" }
                                th { "金额" }
                            }
                        }
                        tbody {
                            for (name , cents) in line_items {
                                if cents != 0 {
                                    tr { key: "line-{name}",
                                        td { "{name}" }
                                        td { class: "is-mono", "{format_money(cents)}" }
                                    }
                                }
                            }
                        }
                        tfoot {
                            tr {
                                td { "本月收费金额" }
                                td { class: "is-mono", "{format_money(bill.total_fee_cents)}" }
                            }
                        }
                    }
                }

                dl { class: "facts",
                    div {
                        dt { "已收" }
                        dd { class: "is-mono is-ok", "{format_money(bill.receipt_amount_cents)}" }
                    }
                    div {
                        dt { "未收" }
                        dd { class: "is-mono", "{format_money(remaining_cents(&bill))}" }
                    }
                    div {
                        dt { "收款日期" }
                        dd { class: "is-mono", "{format_date(bill.receipt_time)}" }
                    }
                    div { class: "is-wide",
                        dt { "对公账户" }
                        dd { "{public_account}" }
                    }
                    div { class: "is-wide",
                        dt { "对私账户" }
                        dd { "{private_account}" }
                    }
                    div { class: "is-wide",
                        dt { "备注" }
                        dd { {bill.remark.as_deref().unwrap_or("--")} }
                    }
                }

                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                    Button { r#type: "button", onclick: move |_| print_window(), "打印账单" }
                }
            }
        }
    }
}
