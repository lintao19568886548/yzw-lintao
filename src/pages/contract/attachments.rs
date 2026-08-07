//! 合同图片本地暂存、预览与 AI 输入准备。

use base64::{engine::general_purpose::STANDARD, Engine};
use dioxus::{html::FileData, prelude::*};

use crate::{
    components::button::{Button, ButtonSize, ButtonVariant},
    services::{validate_business_image, ContractAiImage},
};

#[derive(Clone, PartialEq)]
pub(super) enum ContractAttachment {
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

impl ContractAttachment {
    pub(super) fn preview_url(&self) -> Option<&str> {
        match self {
            Self::Existing { url, .. } => Some(url),
            Self::Pending { preview_url, .. } => preview_url.as_deref(),
        }
    }

    pub(super) fn name(&self) -> String {
        match self {
            Self::Existing { img_id, .. } => format!("合同图片 #{img_id}"),
            Self::Pending { name, .. } => name.clone(),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub(super) fn is_removed(&self) -> bool {
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

pub(super) fn discard_attachment_previews(attachments: &[ContractAttachment]) {
    for attachment in attachments {
        if let ContractAttachment::Pending {
            preview_url: Some(url),
            ..
        } = attachment
        {
            revoke_preview_url(url);
        }
    }
}

/// AI 识别现有 R2 图片和本地待上传图片；此步骤不会把文件写入 R2。
pub(super) async fn build_ai_images(
    attachments: Vec<ContractAttachment>,
) -> Result<Vec<ContractAiImage>, String> {
    let mut images = Vec::new();
    for attachment in attachments.into_iter().filter(|item| !item.is_removed()) {
        match attachment {
            ContractAttachment::Existing { url, .. } => {
                images.push(ContractAiImage { source: url });
            }
            ContractAttachment::Pending { file, .. } => {
                validate_business_image(&file)?;
                let content_type = file.content_type().unwrap_or_else(|| "image/jpeg".into());
                let bytes = file
                    .read_bytes()
                    .await
                    .map_err(|error| format!("读取合同图片失败：{error}"))?;
                images.push(ContractAiImage {
                    source: format!(
                        "data:{content_type};base64,{}",
                        STANDARD.encode(bytes.as_ref())
                    ),
                });
            }
        }
    }
    if images.is_empty() {
        return Err("请先选择至少一张合同图片".into());
    }
    Ok(images)
}

#[component]
pub(super) fn ContractAttachments(
    mut attachments: SyncSignal<Vec<ContractAttachment>>,
    readonly: bool,
    loading: bool,
    recognizing: bool,
    on_recognize: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "business-image-field",
            div { class: "business-image-toolbar",
                div {
                    strong { "合同图片" }
                    small { "支持 JPG、PNG、WebP，单张不超过 10MB，最多 8 张" }
                }
                if !readonly {
                    div { class: "row",
                        label { class: "business-image-upload",
                            input {
                                r#type: "file",
                                accept: "image/jpeg,image/png,image/webp",
                                multiple: true,
                                disabled: loading || recognizing,
                                onchange: move |event| {
                                    let files = event.files();
                                    if files.is_empty() { return; }
                                    let available = 8usize.saturating_sub(attachments.read().len());
                                    if available == 0 {
                                        on_error.call("每份合同最多选择 8 张图片".into());
                                        return;
                                    }
                                    for file in files.into_iter().take(available) {
                                        match validate_business_image(&file) {
                                            Ok(()) => {
                                                let name = file.name();
                                                let preview_url = create_preview_url(&file);
                                                attachments.write().push(ContractAttachment::Pending {
                                                    file,
                                                    name,
                                                    preview_url,
                                                });
                                            }
                                            Err(message) => {
                                                on_error.call(message);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            "选择合同图片"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            r#type: "button",
                            disabled: loading || recognizing
                                || attachments.read().iter().all(ContractAttachment::is_removed),
                            onclick: move |_| on_recognize.call(()),
                            if recognizing {
                                "AI 正在识别…"
                            } else {
                                "AI 识别填表"
                            }
                        }
                    }
                }
            }
            p { class: "hint", "图片选择后仅在浏览器暂存；点击确认保存才上传 R2。AI 识别只补全当前为空的字段，不覆盖已填写内容。" }
            if attachments.read().is_empty() {
                div { class: "business-image-empty", "暂无合同图片" }
            } else {
                div { class: "business-image-list",
                    for (index, attachment) in attachments.read().clone().into_iter().enumerate() {
                        {
                            let url = attachment.preview_url().map(str::to_string);
                            let name = attachment.name();
                            let pending = attachment.is_pending();
                            let removed = attachment.is_removed();
                            rsx! { article {
                                key: "contract-attachment-{index}-{name}",
                                class: if removed { "is-removed" } else { "" },
                                if let Some(url) = url {
                                    a { href: "{url}", target: "_blank", rel: "noreferrer",
                                        img { src: "{url}", alt: "{name}" }
                                    }
                                } else {
                                    div { class: "business-image-placeholder", "图片" }
                                }
                                span { "{name}" }
                                if pending { small { class: "is-pending", "待上传" } }
                                if removed { small { class: "is-removing", "保存后删除" } }
                                if !readonly {
                                    button {
                                        r#type: "button",
                                        disabled: loading || recognizing,
                                        onclick: move |_| {
                                            if pending {
                                                let removed_item = attachments.write().remove(index);
                                                if let ContractAttachment::Pending { preview_url: Some(url), .. } = removed_item {
                                                    revoke_preview_url(&url);
                                                }
                                            } else if let Some(ContractAttachment::Existing { removed, .. }) = attachments.write().get_mut(index) {
                                                *removed = !*removed;
                                            }
                                        },
                                        if removed { "撤销" } else { "移除" }
                                    }
                                }
                            } }
                        }
                    }
                }
            }
        }
    }
}
