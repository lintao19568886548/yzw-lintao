//! 工资记录新增、查看、编辑与删除弹窗。

use dioxus::{html::FileData, prelude::*};

use super::{
    employee_combobox::EmployeeCombobox,
    format::{cents_input_value, format_date, parse_amount_to_cents, parse_date},
};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        ConfirmDialog, DateField,
    },
    services::{
        create_salary_with_images_record, delete_salary_images_from_r2, delete_salary_record,
        update_salary_with_images_record, upload_salary_image, validate_salary_image,
        StoredR2Image,
    },
    spacetime_bindings::{
        employee_type::Employee, salary_image_preview_type::SalaryImagePreview,
        salary_input_type::SalaryInput, salary_type::Salary,
        uploaded_salary_image_input_type::UploadedSalaryImageInput,
    },
};

#[derive(Clone, PartialEq)]
enum SalaryProof {
    Existing {
        img_id: u64,
        url: String,
        removed: bool,
    },
    Pending {
        file: FileData,
        name: String,
        preview_url: Option<String>,
    },
}

impl SalaryProof {
    fn preview_url(&self) -> Option<&str> {
        match self {
            Self::Existing { url, .. } => Some(url),
            Self::Pending { preview_url, .. } => preview_url.as_deref(),
        }
    }

    fn name(&self) -> String {
        match self {
            Self::Existing { img_id, .. } => format!("历史凭证 #{img_id}"),
            Self::Pending { name, .. } => name.clone(),
        }
    }

    fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    fn is_removed(&self) -> bool {
        matches!(self, Self::Existing { removed: true, .. })
    }
}

#[cfg(target_arch = "wasm32")]
fn create_preview_url(file: &FileData) -> Option<String> {
    use dioxus::web::WebFileExt;

    file.get_web_file()
        .and_then(|file| web_sys::Url::create_object_url_with_blob(file.as_ref()).ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn create_preview_url(_file: &FileData) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn revoke_preview_url(url: &str) {
    let _ = web_sys::Url::revoke_object_url(url);
}

#[cfg(not(target_arch = "wasm32"))]
fn revoke_preview_url(_url: &str) {}

fn discard_pending_previews(proofs: &[SalaryProof]) {
    for proof in proofs {
        if let SalaryProof::Pending {
            preview_url: Some(url),
            ..
        } = proof
        {
            revoke_preview_url(url);
        }
    }
}

#[component]
pub fn SalaryFormDialog(
    salary: Option<Salary>,
    employees: Vec<Employee>,
    image_previews: Vec<SalaryImagePreview>,
    readonly: bool,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let salary_id = salary.as_ref().map(|row| row.salary_id);
    let employee_id = use_signal_sync(|| {
        salary
            .as_ref()
            .map(|row| row.employee_id.to_string())
            .unwrap_or_default()
    });
    let mut amount = use_signal_sync(|| {
        salary
            .as_ref()
            .map(|row| cents_input_value(row.salary_amount_cents))
            .unwrap_or_default()
    });
    let mut issue_date = use_signal_sync(|| {
        salary
            .as_ref()
            .map(|row| format_date(row.issue_date))
            .unwrap_or_default()
    });
    let mut issued =
        use_signal_sync(|| salary.as_ref().and_then(|row| row.issued).unwrap_or(false));
    let mut remark = use_signal_sync(|| {
        salary
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let mut completed = use_signal_sync(|| false);
    let mut saving_status = use_signal_sync(String::new);
    let mut proofs = use_signal_sync(|| {
        image_previews
            .iter()
            .map(|image| SalaryProof::Existing {
                img_id: image.img_id,
                url: image.img_url.clone(),
                removed: false,
            })
            .collect::<Vec<_>>()
    });
    use_effect(move || {
        if completed() {
            on_saved.call(());
        }
    });
    let selected_phone = employee_id()
        .parse::<u64>()
        .ok()
        .and_then(|id| employees.iter().find(|employee| employee.employee_id == id))
        .map(|employee| employee.phone.clone())
        .unwrap_or_default();
    let title = if readonly {
        "查看工资记录"
    } else if salary_id.is_some() {
        "编辑工资记录"
    } else {
        "新增工资记录"
    };

    let issued_value: ReadSignal<Option<String>> =
        use_memo(move || Some(if issued() { "true" } else { "false" }.to_string())).into();

    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                // 上传进行中不允许关闭，否则请求还在飞就没人处理结果了。
                if !open && !loading() {
                    discard_pending_previews(&proofs());
                    on_close.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription { "工资金额与发放状态实时同步；凭证在确认保存时才上传。" }
            {
                let form = rsx! {
                form {
                    onsubmit: move |event| {
                        event.prevent_default();
                        if readonly {
                            on_close.call(());
                            return;
                        }
                        if loading() {
                            return;
                        }
                        let Some(employee_id) = employee_id().parse::<u64>().ok() else {
                            error.set(Some("请选择合同人".into()));
                            return;
                        };
                        let salary_amount_cents = match parse_amount_to_cents(&amount()) {
                            Ok(value) => value,
                            Err(message) => { error.set(Some(message)); return; }
                        };
                        let issue_date_value = match parse_date(&issue_date()) {
                            Ok(value) => value,
                            Err(message) => { error.set(Some(message)); return; }
                        };
                        if remark().chars().count() > 300 {
                            error.set(Some("备注不能超过 300 个字符".into()));
                            return;
                        }
                        error.set(None);
                        let input = SalaryInput {
                            employee_id,
                            salary_amount_cents: Some(salary_amount_cents),
                            issue_date: Some(issue_date_value),
                            issued: Some(issued()),
                            remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                        };
                        let proof_values = proofs();
                        let existing_image_ids = proof_values.iter().filter_map(|proof| match proof {
                            SalaryProof::Existing { img_id, removed: false, .. } => Some(*img_id),
                            SalaryProof::Existing { removed: true, .. } => None,
                            SalaryProof::Pending { .. } => None,
                        }).collect::<Vec<_>>();
                        let removed_images = proof_values.iter().filter_map(|proof| match proof {
                            SalaryProof::Existing { img_id, url, removed: true } => Some(StoredR2Image {
                                img_id: *img_id,
                                public_url: url.clone(),
                            }),
                            _ => None,
                        }).collect::<Vec<_>>();
                        let pending_files = proof_values.into_iter().filter_map(|proof| match proof {
                            SalaryProof::Pending { file, .. } => Some(file),
                            SalaryProof::Existing { .. } => None,
                        }).collect::<Vec<_>>();
                        loading.set(true);
                        saving_status.set(if pending_files.is_empty() { "正在保存记录…".into() } else { format!("正在上传 0/{}…", pending_files.len()) });
                        spawn(async move {
                            let total = pending_files.len();
                            let mut uploads = Vec::with_capacity(total);
                            for (index, file) in pending_files.into_iter().enumerate() {
                                saving_status.set(format!("正在上传 {}/{}…", index + 1, total));
                                match upload_salary_image(file).await {
                                    Ok(image) => uploads.push(UploadedSalaryImageInput {
                                        img_url: image.public_url,
                                        hash: image.sha256,
                                    }),
                                    Err(message) => {
                                        loading.set(false);
                                        saving_status.set(String::new());
                                        error.set(Some(message));
                                        return;
                                    }
                                }
                            }
                            saving_status.set("正在写入数据库…".into());
                            let result = if let Some(id) = salary_id {
                                update_salary_with_images_record(id, input, existing_image_ids, uploads).await
                            } else {
                                create_salary_with_images_record(input, uploads).await
                            };
                            match result {
                                Ok(()) => {
                                    saving_status.set("正在清理已移除凭证…".into());
                                    match delete_salary_images_from_r2(removed_images).await {
                                        Ok(_) => {
                                            loading.set(false);
                                            saving_status.set(String::new());
                                            discard_pending_previews(&proofs());
                                            completed.set(true);
                                        }
                                        Err(message) => {
                                            loading.set(false);
                                            saving_status.set(String::new());
                                            error.set(Some(format!("数据库已保存，但{message}；请再次确认保存以重试")));
                                        }
                                    }
                                }
                                Err(message) => {
                                    loading.set(false);
                                    saving_status.set(String::new());
                                    error.set(Some(message));
                                }
                            }
                        });
                    },
                    div { class: "form-grid",
                        div { class: "field",
                            span { class: "field-label", "员工" }
                            EmployeeCombobox {
                                employees: employees.clone(),
                                selected_id: employee_id,
                                readonly,
                            }
                        }
                        div { class: "field",
                            Label { html_for: "salary-phone-display", "联系电话" }
                            Input {
                                id: "salary-phone-display",
                                value: "{selected_phone}",
                                readonly: true,
                                placeholder: "选择合同人后自动带出",
                            }
                        }
                        div { class: "field",
                            Label { html_for: "salary-amount", "工资金额（元）" }
                            Input {
                                id: "salary-amount",
                                value: amount(),
                                readonly,
                                inputmode: "decimal",
                                placeholder: "0.00",
                                oninput: move |event: FormEvent| amount.set(event.value()),
                            }
                        }
                        div { class: "field",
                            span { class: "field-label", "发放日期" }
                            DateField {
                                value: issue_date(),
                                disabled: readonly,
                                on_change: move |value: String| issue_date.set(value),
                            }
                        }
                        div { class: "field",
                            Label { html_for: "salary-issued-select", "发放状态" }
                            Select {
                                id: "salary-issued-select",
                                value: Some(issued_value),
                                disabled: readonly,
                                on_value_change: move |value: Option<String>| {
                                    issued.set(value.as_deref() == Some("true"));
                                },
                                SelectOption::<String> { value: "false".to_string(), index: 0usize, text_value: "待发放".to_string(), "待发放" }
                                SelectOption::<String> { value: "true".to_string(), index: 1usize, text_value: "已发放".to_string(), "已发放" }
                            }
                        }
                        div { class: "field is-wide",
                            Label { html_for: "salary-remark", "备注" }
                            Textarea {
                                id: "salary-remark",
                                value: remark(),
                                readonly,
                                maxlength: 300,
                                rows: 4,
                                placeholder: "填写工资说明或发放备注",
                                oninput: move |event: FormEvent| remark.set(event.value()),
                            }
                            small { class: "hint", "{remark().chars().count()}/300" }
                        }
                        div { class: "field is-wide business-image-field",
                            div { class: "business-image-toolbar",
                                strong { "工资凭证" }
                                if !readonly {
                                    label { class: "business-image-upload",
                                        input {
                                            r#type: "file",
                                            accept: "image/jpeg,image/png,image/webp",
                                            multiple: true,
                                            disabled: loading(),
                                            onchange: move |event| {
                                                let files = event.files();
                                                if files.is_empty() { return; }
                                                error.set(None);
                                                for file in files {
                                                    match validate_salary_image(&file) {
                                                        Ok(()) => {
                                                            let name = file.name();
                                                            let preview_url = create_preview_url(&file);
                                                            proofs.write().push(SalaryProof::Pending {
                                                                file,
                                                                name,
                                                                preview_url,
                                                            });
                                                        }
                                                        Err(message) => { error.set(Some(message)); break; }
                                                    }
                                                }
                                            }
                                        }
                                        "选择工资凭证"
                                    }
                                }
                            }
                            p { class: "hint", "支持 JPG、PNG、WebP，单张不超过 10MB；选择后仅在本地暂存，点击确认保存才上传到 R2。" }
                            if proofs().is_empty() {
                                p { class: "business-image-empty", "暂无工资凭证" }
                            } else {
                                div { class: "business-image-list",
                                    for (index, proof) in proofs().into_iter().enumerate() {
                                        {
                                            let preview_url = proof.preview_url().map(str::to_string);
                                            let name = proof.name();
                                            let is_pending = proof.is_pending();
                                            let is_removed = proof.is_removed();
                                            rsx! { article {
                                                key: "proof-{index}-{name}",
                                                class: if is_removed { "is-removed" } else { "" },
                                                if let Some(url) = preview_url {
                                                    a { href: "{url}", target: "_blank", rel: "noreferrer",
                                                        img { src: "{url}", alt: "{name}" }
                                                    }
                                                } else {
                                                    div { class: "business-image-placeholder", "图片" }
                                                }
                                                span { "{name}" }
                                                if is_pending { small { "待上传" } }
                                                if is_removed { small { class: "is-removing", "确认后删除" } }
                                                if !readonly {
                                                    button { r#type: "button", disabled: loading(), onclick: move |_| {
                                                        if is_pending {
                                                            let removed = proofs.write().remove(index);
                                                            if let SalaryProof::Pending { preview_url: Some(url), .. } = removed {
                                                                revoke_preview_url(&url);
                                                            }
                                                        } else if let Some(SalaryProof::Existing { removed, .. }) = proofs.write().get_mut(index) {
                                                            *removed = !*removed;
                                                        }
                                                    }, if is_removed { "撤销" } else { "移除" } }
                                                }
                                            } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(message) = error() {
                        p { class: "form-error", role: "alert", "{message}" }
                    }
                    div { class: "form-actions",
                        if readonly {
                            Button { r#type: "button", onclick: move |_| on_close.call(()), "关闭" }
                        } else {
                            Button {
                                variant: ButtonVariant::Outline,
                                r#type: "button",
                                disabled: loading(),
                                onclick: move |_| {
                                    discard_pending_previews(&proofs());
                                    on_close.call(());
                                },
                                "取消"
                            }
                            Button {
                                r#type: "submit",
                                disabled: loading(),
                                if loading() {
                                    "{saving_status}"
                                } else {
                                    "确认保存"
                                }
                            }
                        }
                    }
                }
                };
                form
            }
        }
    }
}

#[component]
pub fn SalaryDeleteDialog(
    salary: Salary,
    tenant_name: String,
    images: Vec<StoredR2Image>,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);
    // 数据库删除成功但 R2 清理失败时，重试只需要重跑清理那一步。
    let mut database_deleted = use_signal(|| false);
    let salary_id = salary.salary_id;

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    rsx! {
        ConfirmDialog {
            title: "删除工资记录",
            description: format!(
                "确定永久删除 {tenant_name} 的这条工资记录吗？工资主记录、图片关系和不再被其他业务使用的 R2 凭证都会被删除，此操作不可恢复。",
            ),
            confirm_label: if database_deleted() { "重试清理" } else { "确认永久删除" },
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                let cleanup_images = images.clone();
                spawn(async move {
                    if !database_deleted() {
                        match delete_salary_record(salary_id).await {
                            Ok(()) => database_deleted.set(true),
                            Err(message) => {
                                loading.set(false);
                                error.set(Some(message));
                                return;
                            }
                        }
                    }
                    match delete_salary_images_from_r2(cleanup_images).await {
                        Ok(_) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(format!("工资记录已删除，但{message}；请重试清理")));
                        }
                    }
                });
            },
        }
    }
}
