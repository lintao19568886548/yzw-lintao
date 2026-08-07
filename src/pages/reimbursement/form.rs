//! 报销申请填写与确认上传弹窗。

use dioxus::prelude::*;

use super::{
    attachments::{discard_previews, PendingReimbursementImage, ReimbursementAttachments},
    model::{parse_amount, parse_date},
};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        DateField,
    },
    services::{create_reimbursement_with_images_record, upload_business_image},
    spacetime_bindings::{
        park_type::Park, reimbursement_input_type::ReimbursementInput,
        uploaded_reimbursement_image_input_type::UploadedReimbursementImageInput,
    },
};

#[component]
pub(super) fn ReimbursementFormDialog(
    parks: Vec<Park>,
    applicant: String,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let mut purpose = use_signal(String::new);
    let mut amount = use_signal(String::new);
    let mut payee = use_signal(|| applicant.clone());
    let mut date = use_signal(crate::pages::smart_meter::date::today);
    let mut department = use_signal(String::new);
    let mut park_id = use_signal(|| {
        parks
            .first()
            .map(|park| park.park_id.to_string())
            .unwrap_or_default()
    });
    let mut remark = use_signal(String::new);
    let images = use_signal_sync(Vec::<PendingReimbursementImage>::new);
    let mut loading = use_signal(|| false);
    let mut progress = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                // 上传进行中不允许关闭，否则请求还在飞就没人处理结果了。
                if !open && !loading() {
                    discard_previews(&images());
                    on_close.call(());
                }
            },
            DialogTitle { "新增报销申请" }
            DialogDescription { "申请人 {applicant} · 提交后进入园区审核队列" }

            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if loading() {
                        return;
                    }
                    let purpose_value = purpose().trim().to_string();
                    let payee_value = payee().trim().to_string();
                    if purpose_value.is_empty() {
                        error.set(Some("请输入报销事由".into()));
                        return;
                    }
                    if purpose_value.chars().count() > 100 {
                        error.set(Some("报销事由不能超过 100 个字符".into()));
                        return;
                    }
                    if payee_value.is_empty() {
                        error.set(Some("请输入收款人".into()));
                        return;
                    }
                    let amount_cents = match parse_amount(&amount()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    let reimbursement_date = match parse_date(&date()) {
                        Ok(value) => value,
                        Err(message) => {
                            error.set(Some(message));
                            return;
                        }
                    };
                    let Some(selected_park) = park_id().parse::<u64>().ok() else {
                        error.set(Some("请选择所属园区".into()));
                        return;
                    };
                    if remark().chars().count() > 200 {
                        error.set(Some("备注不能超过 200 个字符".into()));
                        return;
                    }
                    let input = ReimbursementInput {
                        purpose: purpose_value,
                        amount_cents,
                        payee: payee_value,
                        reimbursement_date: Some(reimbursement_date),
                        department: (!department().trim().is_empty())
                            .then(|| department().trim().to_string()),
                        remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                        park_id: selected_park,
                        image_ids: Vec::new(),
                    };
                    let files = images().into_iter().map(|image| image.file).collect::<Vec<_>>();
                    loading.set(true);
                    error.set(None);
                    progress
                        .set(
                            if files.is_empty() {
                                "正在提交申请…".into()
                            } else {
                                format!("准备上传 0/{} 张凭证…", files.len())
                            },
                        );
                    spawn(async move {
                        let total = files.len();
                        let mut uploads = Vec::with_capacity(total);
                        for (index, file) in files.into_iter().enumerate() {
                            progress.set(format!("正在上传 {}/{} 张凭证…", index + 1, total));
                            match upload_business_image(file).await {
                                Ok(image) => {
                                    uploads
                                        .push(UploadedReimbursementImageInput {
                                            img_url: image.public_url,
                                            hash: image.sha256,
                                        })
                                }
                                Err(message) => {
                                    loading.set(false);
                                    progress.set(String::new());
                                    error.set(Some(message));
                                    return;
                                }
                            }
                        }
                        progress.set("正在写入审核队列…".into());
                        match create_reimbursement_with_images_record(input, uploads).await {
                            Ok(()) => {
                                discard_previews(&images());
                                completed.set(true);
                            }
                            Err(message) => {
                                loading.set(false);
                                progress.set(String::new());
                                error.set(Some(message));
                            }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field is-wide",
                        Label { html_for: "expense-purpose", "报销事由 *" }
                        Input {
                            id: "expense-purpose",
                            value: purpose(),
                            maxlength: 100,
                            placeholder: "例如：园区设备维修材料费",
                            disabled: loading(),
                            oninput: move |event: FormEvent| purpose.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "expense-amount", "报销金额（元）*" }
                        Input {
                            id: "expense-amount",
                            value: amount(),
                            inputmode: "decimal",
                            placeholder: "0.00",
                            disabled: loading(),
                            oninput: move |event: FormEvent| amount.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "expense-payee", "收款人 *" }
                        Input {
                            id: "expense-payee",
                            value: payee(),
                            maxlength: 50,
                            disabled: loading(),
                            oninput: move |event: FormEvent| payee.set(event.value()),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "报销日期 *" }
                        DateField {
                            value: date(),
                            disabled: loading(),
                            on_change: move |value: String| date.set(value),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "expense-park-select", "所属园区 *" }
                        Select {
                            id: "expense-park-select",
                            value: Some(park_value),
                            disabled: loading(),
                            on_value_change: move |value: Option<String>| park_id.set(value.unwrap_or_default()),
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "expense-form-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "expense-department", "部门" }
                        Input {
                            id: "expense-department",
                            value: department(),
                            maxlength: 50,
                            placeholder: "选填",
                            disabled: loading(),
                            oninput: move |event: FormEvent| department.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "expense-remark", "备注" }
                        Textarea {
                            id: "expense-remark",
                            value: remark(),
                            maxlength: 200,
                            rows: 3,
                            placeholder: "填写费用说明、付款信息或审核补充",
                            disabled: loading(),
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                }

                ReimbursementAttachments {
                    images,
                    loading: loading(),
                    on_error: move |message| error.set(Some(message)),
                }

                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                if !progress().is_empty() {
                    p { class: "notice", role: "status", "{progress}" }
                }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| {
                            discard_previews(&images());
                            on_close.call(());
                        },
                        "取消"
                    }
                    Button {
                        r#type: "submit",
                        disabled: loading(),
                        if loading() {
                            "正在提交…"
                        } else {
                            "确认提交"
                        }
                    }
                }
            }
        }
    }
}
