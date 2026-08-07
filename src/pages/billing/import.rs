//! 新增方式选择与 AI Excel 文件读取弹窗。

use dioxus::prelude::*;

use crate::{
    components::{
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
    },
    services::{analyze_amount_bill_excel, validate_amount_bill_excel, AiAmountBillDraft},
};

#[component]
pub(super) fn BillCreateModeDialog(
    on_manual: EventHandler<()>,
    on_ai: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "新增总账单" }
            DialogDescription { "选择手动录入，或先让 AI 识别 Excel 后逐份核对。" }

            div { class: "grid-2",
                button { class: "tile", r#type: "button", onclick: move |_| on_ai.call(()),
                    span { class: "tile-label", "AI Excel 自动导入" }
                    small { class: "hint", "上传 .xlsx，识别后不会直接写入数据库" }
                    small { class: "hint", "识别 → 人工核对 → 逐份保存" }
                }
                button { class: "tile", r#type: "button", onclick: move |_| on_manual.call(()),
                    span { class: "tile-label", "手动录入" }
                    small { class: "hint", "打开空白总账单录入表单" }
                    small { class: "hint", "适合单份账单或临时补录" }
                }
            }
        }
    }
}

#[component]
pub(super) fn BillAiImportDialog(
    on_close: EventHandler<()>,
    on_analyzed: EventHandler<Vec<AiAmountBillDraft>>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut file_name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { "AI Excel 自动导入" }
            DialogDescription { "仅支持 .xlsx，单个文件不超过 20MB。识别结果必须人工确认。" }

            div { class: "stack",
                Card {
                    CardContent {
                        div { class: "stack",
                            label { class: "business-image-upload",
                                input {
                                    r#type: "file",
                                    accept: ".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                                    disabled: loading(),
                                    onchange: move |event| {
                                        let Some(file) = event.files().into_iter().next() else {
                                            return;
                                        };
                                        file_name.set(file.name());
                                        if let Err(message) = validate_amount_bill_excel(&file) {
                                            error.set(Some(message));
                                            return;
                                        }
                                        loading.set(true);
                                        error.set(None);
                                        spawn(async move {
                                            match analyze_amount_bill_excel(file).await {
                                                Ok(drafts) => on_analyzed.call(drafts),
                                                Err(message) => {
                                                    loading.set(false);
                                                    error.set(Some(message));
                                                }
                                            }
                                        });
                                    },
                                }
                                if loading() {
                                    "正在读取并识别账单…"
                                } else {
                                    "选择 Excel 文件"
                                }
                            }
                            p { class: "hint",
                                if file_name().is_empty() {
                                    "文件只用于本次识别，不会自动保存账单"
                                } else {
                                    "{file_name}"
                                }
                            }
                        }
                    }
                }

                dl { class: "facts",
                    div {
                        dt { "第一步" }
                        dd { "读取工作表" }
                    }
                    div {
                        dt { "第二步" }
                        dd { "AI 拆分账单" }
                    }
                    div {
                        dt { "第三步" }
                        dd { "逐份核对保存" }
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
                }
            }
        }
    }
}
