//! 园区档案表单：新增与编辑共用同一个对话框。
//!
//! 新增和编辑曾经是两份各写一遍的表单，结果编辑那份加了园区图片、新增那份
//! 没有——同一张主档，从园区管理进去能传图，从新增进去传不了。现在只有这
//! 一个组件，`park` 传 `None` 表示建档，字段和图片区自然不可能再漂移。

use dioxus::prelude::*;

use crate::{
    components::{
        button::{Button, ButtonVariant},
        dialog::{Dialog, DialogDescription, DialogTitle},
        discard_image_previews,
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        BusinessImage, ImageEditor,
    },
    services::{
        create_park_with_images_record, delete_business_images_from_r2,
        update_park_with_images_record, upload_business_image, StoredR2Image,
    },
    spacetime_bindings::{
        park_image_preview_type::ParkImagePreview, park_input_type::ParkInput, park_type::Park,
        uploaded_park_image_input_type::UploadedParkImageInput,
    },
};

use super::model::park_is_enabled;

/// 与服务端 `MAX_PARK_IMAGES` 保持一致。
const MAX_PARK_IMAGES: usize = 8;

/// 园区主档表单。`park` 为 `None` 时是新增，否则是编辑该园区。
#[component]
pub(crate) fn ParkProfileDialog(
    park: Option<Park>,
    #[props(default)] previews: Vec<ParkImagePreview>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let park_id = park.as_ref().map(|park| park.park_id);
    let mut park_name = use_signal_sync(|| {
        park.as_ref()
            .map(|park| park.park_name.clone())
            .unwrap_or_default()
    });
    let mut address = use_signal_sync(|| {
        park.as_ref()
            .map(|park| park.address.clone())
            .unwrap_or_default()
    });
    let mut manager = use_signal_sync(|| {
        park.as_ref()
            .and_then(|park| park.manager.clone())
            .unwrap_or_default()
    });
    let mut contact = use_signal_sync(|| {
        park.as_ref()
            .and_then(|park| park.contact.clone())
            .unwrap_or_default()
    });
    let mut status = use_signal_sync(|| {
        // 新建默认启用，编辑沿用现状。
        let enabled = park
            .as_ref()
            .is_none_or(|park| park_is_enabled(park.status.as_deref()));
        if enabled { "1".to_string() } else { "0".to_string() }
    });
    let mut description = use_signal_sync(|| {
        park.as_ref()
            .and_then(|park| park.description.clone())
            .unwrap_or_default()
    });
    let images = use_signal_sync(|| {
        previews
            .iter()
            .map(|preview| BusinessImage::Existing {
                img_id: preview.img_id,
                url: preview.img_url.clone(),
                removed: false,
            })
            .collect::<Vec<_>>()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    // Select 的受控值要求 ReadSignal<Option<T>>，这里把状态信号映射过去。
    let status_value: ReadSignal<Option<String>> = use_memo(move || Some(status())).into();

    rsx! {
        Dialog {
            open: true,
            is_modal: true,
            on_open_change: move |open: bool| {
                // 保存过程中不允许 Esc 或遮罩关闭，否则请求还在飞就没人处理结果了。
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { if park_id.is_some() { "编辑园区档案" } else { "新增园区" } }
            DialogDescription { "基本信息与园区图片一并保存，图片在确认后才会上传。" }
            form {
                onsubmit: move |event| {
                    event.prevent_default();
                    if loading() { return; }
                    let name = park_name().trim().to_string();
                    let address_value = address().trim().to_string();
                    let manager_value = manager().trim().to_string();
                    let contact_value = contact().trim().to_string();
                    if name.is_empty() { error.set(Some("请输入园区名称".into())); return; }
                    if address_value.is_empty() { error.set(Some("请输入园区地址".into())); return; }
                    if manager_value.is_empty() { error.set(Some("请输入园区负责人".into())); return; }
                    if contact_value.is_empty() { error.set(Some("请输入负责人联系方式".into())); return; }
                    if description().chars().count() > 300 { error.set(Some("园区说明不能超过 300 个字符".into())); return; }
                    let input = ParkInput {
                        park_name: name,
                        address: address_value,
                        manager: Some(manager_value),
                        contact: Some(contact_value),
                        status: Some(status()),
                        description: (!description().trim().is_empty()).then(|| description().trim().to_string()),
                    };

                    let current = images();
                    let existing_image_ids = current.iter().filter_map(|image| match image {
                        BusinessImage::Existing { img_id, removed: false, .. } => Some(*img_id),
                        _ => None,
                    }).collect::<Vec<_>>();
                    // 被标记移除的图片先解绑，服务端确认成功后再删 R2 对象。
                    let removed_images = current.iter().filter_map(|image| match image {
                        BusinessImage::Existing { img_id, url, removed: true } => Some(StoredR2Image { img_id: *img_id, public_url: url.clone() }),
                        _ => None,
                    }).collect::<Vec<_>>();
                    let pending_files = current.into_iter().filter_map(|image| match image {
                        BusinessImage::Pending { file, .. } => Some(file),
                        _ => None,
                    }).collect::<Vec<_>>();

                    error.set(None);
                    loading.set(true);
                    spawn(async move {
                        let mut uploads = Vec::with_capacity(pending_files.len());
                        for file in pending_files {
                            match upload_business_image(file).await {
                                Ok(image) => uploads.push(UploadedParkImageInput { img_url: image.public_url, hash: image.sha256 }),
                                Err(message) => { loading.set(false); error.set(Some(message)); return; }
                            }
                        }
                        // 建档时自增主键要落库之后才拿得到，图片只能跟主档一起提交，
                        // 由服务端在同一事务里写完，不存在“园区建好了图片没绑上”的中间态。
                        let saved = match park_id {
                            Some(park_id) => update_park_with_images_record(park_id, input, existing_image_ids, uploads).await,
                            None => create_park_with_images_record(input, uploads).await,
                        };
                        match saved {
                            Ok(()) => {
                                let _ = delete_business_images_from_r2(removed_images).await;
                                discard_image_previews(&images());
                                loading.set(false);
                                on_saved.call(());
                            }
                            Err(message) => { loading.set(false); error.set(Some(message)); }
                        }
                    });
                },
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "park-form-name", "园区名称" }
                        Input { id: "park-form-name", value: park_name(), maxlength: 100, placeholder: "例如：十一高步园区", oninput: move |event: FormEvent| park_name.set(event.value()) }
                    }
                    div { class: "field",
                        Label { html_for: "park-form-status", "园区状态" }
                        Select {
                            id: "park-form-status",
                            value: Some(status_value),
                            on_value_change: move |value: Option<String>| {
                                if let Some(value) = value { status.set(value); }
                            },
                            SelectOption::<String> { value: "1".to_string(), index: 0usize, text_value: "启用".to_string(), "启用" }
                            SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "停用".to_string(), "停用" }
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "park-form-address", "详细地址" }
                        Input { id: "park-form-address", value: address(), maxlength: 200, placeholder: "请输入园区详细地址", oninput: move |event: FormEvent| address.set(event.value()) }
                    }
                    div { class: "field",
                        Label { html_for: "park-form-manager", "负责人" }
                        Input { id: "park-form-manager", value: manager(), maxlength: 50, placeholder: "请输入负责人姓名", oninput: move |event: FormEvent| manager.set(event.value()) }
                    }
                    div { class: "field",
                        Label { html_for: "park-form-contact", "联系方式" }
                        Input { id: "park-form-contact", inputmode: "tel", value: contact(), maxlength: 50, placeholder: "手机或座机", oninput: move |event: FormEvent| contact.set(event.value()) }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "park-form-description", "园区说明" }
                        Textarea { id: "park-form-description", value: description(), maxlength: 300, rows: 3, placeholder: "补充园区定位、交通或经营说明", oninput: move |event: FormEvent| description.set(event.value()) }
                        small { class: "hint", "{description().chars().count()}/300" }
                    }
                }
                div { class: "field is-wide",
                    ImageEditor {
                        images,
                        title: "园区图片".to_string(),
                        max_count: MAX_PARK_IMAGES,
                        loading: loading(),
                        on_error: move |message| error.set(Some(message)),
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", "{message}" }
                }
                footer { class: "form-actions",
                    Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_close.call(()), "取消" }
                    Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
                }
            }
        }
    }
}
