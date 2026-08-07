//! 报销凭证在浏览器中的本地暂存与预览。

use dioxus::{html::FileData, prelude::*};

use crate::services::validate_business_image;

#[derive(Clone, PartialEq)]
pub(super) struct PendingReimbursementImage {
    pub file: FileData,
    pub name: String,
    pub preview_url: Option<String>,
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

pub(super) fn discard_previews(images: &[PendingReimbursementImage]) {
    for image in images {
        if let Some(url) = &image.preview_url {
            revoke_preview_url(url);
        }
    }
}

#[component]
pub(super) fn ReimbursementAttachments(
    mut images: SyncSignal<Vec<PendingReimbursementImage>>,
    loading: bool,
    on_error: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "business-image-field",
            div { class: "business-image-toolbar",
                div { strong { "报销凭证" } small { "JPG、PNG、WebP · 单张不超过 10MB · 最多 8 张" } }
                label { class: "business-image-upload",
                    input {
                        r#type: "file", accept: "image/jpeg,image/png,image/webp", multiple: true, disabled: loading,
                        onchange: move |event| {
                            let files = event.files();
                            let available = 8usize.saturating_sub(images.read().len());
                            if available == 0 { on_error.call("每份报销申请最多选择 8 张凭证".into()); return; }
                            for file in files.into_iter().take(available) {
                                match validate_business_image(&file) {
                                    Ok(()) => {
                                        let name = file.name();
                                        let preview_url = create_preview_url(&file);
                                        images.write().push(PendingReimbursementImage { file, name, preview_url });
                                    }
                                    Err(message) => { on_error.call(message); break; }
                                }
                            }
                        }
                    }
                    "+ 选择凭证"
                }
            }
            p { class: "hint", "选择后仅保存在当前浏览器；点击“确认提交”后才开始上传 R2，取消不会产生远端文件。" }
            if images.read().is_empty() {
                div { class: "business-image-empty", "暂无报销凭证" }
            } else {
                div { class: "business-image-list",
                    for (index, image) in images.read().clone().into_iter().enumerate() {
                        article { key: "expense-proof-{index}-{image.name}",
                            if let Some(url) = image.preview_url.clone() { img { src: "{url}", alt: "{image.name}" } }
                            else { div { class: "business-image-placeholder", "图片" } }
                            span { "{image.name}" }
                            small { "待上传" }
                            button { r#type: "button", disabled: loading, onclick: move |_| {
                                let removed = images.write().remove(index);
                                if let Some(url) = removed.preview_url { revoke_preview_url(&url); }
                            }, "移除" }
                        }
                    }
                }
            }
        }
    }
}
