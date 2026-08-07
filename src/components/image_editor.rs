//! 业务图片的本地暂存、预览与增删组件。
//!
//! 园区、楼层、宿舍等资产档案共用同一套图片编辑交互：既有图片标记移除、
//! 新选图片先在本地预览，直到用户确认保存才真正上传到 R2。
//!
//! 合同表单另有一套带 AI 识别的实现（`pages/contract/attachments.rs`），
//! 那里的识别流程与本组件职责不同，暂不合并。

use dioxus::{html::FileData, prelude::*};

use crate::services::validate_business_image;

/// 编辑中的一张业务图片。
#[derive(Clone, PartialEq)]
pub enum BusinessImage {
    /// 已保存在 R2 且被业务记录引用的图片。
    Existing {
        img_id: u64,
        url: String,
        removed: bool,
    },
    /// 用户刚选择、尚未上传的本地文件。
    Pending {
        file: FileData,
        name: String,
        preview_url: Option<String>,
    },
}

impl BusinessImage {
    pub fn preview_url(&self) -> Option<&str> {
        match self {
            Self::Existing { url, .. } => Some(url),
            Self::Pending { preview_url, .. } => preview_url.as_deref(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Existing { img_id, .. } => format!("图片 #{img_id}"),
            Self::Pending { name, .. } => name.clone(),
        }
    }

    pub fn is_removed(&self) -> bool {
        matches!(self, Self::Existing { removed: true, .. })
    }

    /// 保存后仍然保留的图片数量，用于校验上限。
    pub fn retained_count(images: &[Self]) -> usize {
        images.iter().filter(|image| !image.is_removed()).count()
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

/// 释放本地预览占用的对象 URL，避免浏览器内存泄漏。
pub fn discard_image_previews(images: &[BusinessImage]) {
    for image in images {
        if let BusinessImage::Pending {
            preview_url: Some(url),
            ..
        } = image
        {
            revoke_preview_url(url);
        }
    }
}

#[component]
pub fn ImageEditor(
    mut images: SyncSignal<Vec<BusinessImage>>,
    title: String,
    max_count: usize,
    loading: bool,
    on_error: EventHandler<String>,
) -> Element {
    let current = images();
    let retained = BusinessImage::retained_count(&current);

    rsx! {
        div { class: "business-image-field",
            div { class: "business-image-toolbar",
                div {
                    strong { "{title}" }
                    small { "支持 JPG、PNG、WebP，单张不超过 10MB，最多 {max_count} 张" }
                }
                label { class: "business-image-upload",
                    input {
                        r#type: "file",
                        accept: "image/jpeg,image/png,image/webp",
                        multiple: true,
                        disabled: loading,
                        onchange: move |event| {
                            let mut next = images();
                            for file in event.files() {
                                if BusinessImage::retained_count(&next) >= max_count {
                                    on_error.call(format!("最多只能保留 {max_count} 张图片"));
                                    break;
                                }
                                if let Err(message) = validate_business_image(&file) {
                                    on_error.call(message);
                                    continue;
                                }
                                let name = file.name();
                                let preview_url = create_preview_url(&file);
                                next.push(BusinessImage::Pending { file, name, preview_url });
                            }
                            images.set(next);
                        },
                    }
                    span { if loading { "上传中…" } else { "选择图片" } }
                }
            }
            if current.is_empty() {
                p { class: "business-image-empty", "尚未添加图片" }
            } else {
                ul { class: "business-image-list",
                    for (index, image) in current.into_iter().enumerate() {
                        li {
                            key: "{index}",
                            class: if image.is_removed() { "is-removed" } else { "" },
                            if let Some(url) = image.preview_url() {
                                img { src: "{url}", alt: "{image.name()}" }
                            } else {
                                span { class: "business-image-placeholder", "无预览" }
                            }
                            div { class: "business-image-meta",
                                span { "{image.name()}" }
                                if image.is_removed() { small { "保存后移除" } }
                            }
                            button {
                                r#type: "button",
                                disabled: loading,
                                onclick: move |_| {
                                    let mut next = images();
                                    match next.get(index) {
                                        // 既有图片只做标记，真正解绑发生在保存时。
                                        Some(BusinessImage::Existing { .. }) => {
                                            if let Some(BusinessImage::Existing { removed, .. }) = next.get_mut(index) {
                                                *removed = !*removed;
                                            }
                                        }
                                        // 尚未上传的本地文件直接丢弃，并释放预览 URL。
                                        Some(BusinessImage::Pending { .. }) => {
                                            let removed = next.remove(index);
                                            discard_image_previews(std::slice::from_ref(&removed));
                                        }
                                        None => return,
                                    }
                                    images.set(next);
                                },
                                if image.is_removed() { "撤销" } else { "移除" }
                            }
                        }
                    }
                }
                p { class: "business-image-count", "保存后保留 {retained} / {max_count} 张" }
            }
        }
    }
}
