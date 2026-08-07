//! 厂房与楼层的区块内联编辑。
//!
//! 表单直接在资产卡片内展开，而不是像 `/rental/manage` 那样层层弹窗——
//! 园区 → 厂房面板 → 厂房表单 → 楼层表单四层叠加在移动端几乎无法操作。

use dioxus::prelude::*;

use super::asset_form::{decimal_input, parse_decimal};
use crate::{
    components::{
        button::{Button, ButtonVariant},
        discard_image_previews,
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        BusinessImage, DateField, ImageEditor,
    },
    services::{
        create_dormitory_floor_record, create_dormitory_record, create_factory_floor_record,
        create_factory_with_floors_record, update_dormitory_floor_record,
        delete_business_images_from_r2, update_dormitory_with_images_record,
        update_factory_floor_with_images_record, update_factory_record, upload_business_image,
        StoredR2Image,
    },
    spacetime_bindings::{
        dormitory_floor_input_type::DormitoryFloorInput, dormitory_floor_type::DormitoryFloor,
        dormitory_image_preview_type::DormitoryImagePreview, dormitory_input_type::DormitoryInput,
        dormitory_type::Dormitory, factory_floor_image_preview_type::FactoryFloorImagePreview,
        factory_floor_input_type::FactoryFloorInput, factory_floor_type::FactoryFloor,
        factory_input_type::FactoryInput, factory_type::Factory,
        uploaded_dormitory_image_input_type::UploadedDormitoryImageInput,
        uploaded_floor_image_input_type::UploadedFloorImageInput,
    },
};

/// 与服务端 `MAX_FLOOR_IMAGES` 保持一致。
const MAX_FLOOR_IMAGES: usize = 8;
/// 与服务端 `MAX_DORMITORY_IMAGES` 保持一致。
const MAX_DORMITORY_IMAGES: usize = 8;

/// 解析房间数、层数这类非负整数输入。
fn parse_count(value: &str, label: &str) -> Result<i32, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    value
        .parse::<i32>()
        .ok()
        .filter(|parsed| *parsed >= 0)
        .ok_or_else(|| format!("{label}必须是非负整数"))
}

/// 厂房新增与编辑的内联表单。
#[component]
pub(super) fn FactoryInlineForm(
    park_id: u64,
    factory: Option<Factory>,
    on_done: EventHandler<()>,
) -> Element {
    let factory_id = factory.as_ref().map(|row| row.factory_id);
    let mut factory_name = use_signal_sync(|| {
        factory
            .as_ref()
            .map(|row| row.factory_name.clone())
            .unwrap_or_default()
    });
    let mut build_date = use_signal_sync(|| {
        factory
            .as_ref()
            .and_then(|row| row.build_date.clone())
            .unwrap_or_default()
    });
    let mut is_own = use_signal_sync(|| factory.as_ref().is_none_or(|row| row.is_own));
    let mut description = use_signal_sync(|| {
        factory
            .as_ref()
            .and_then(|row| row.description.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let own_value: ReadSignal<Option<String>> = use_memo(move || {
        Some(if is_own() {
            "1".to_string()
        } else {
            "0".to_string()
        })
    })
    .into();

    let title = if factory_id.is_some() {
        "编辑厂房"
    } else {
        "新增厂房"
    };

    rsx! {
        form { class: "inline-form",
            onsubmit: move |event| {
                event.prevent_default();
                if loading() { return; }
                let name = factory_name().trim().to_string();
                if name.is_empty() { error.set(Some("请输入厂房名称".into())); return; }
                let input = FactoryInput {
                    factory_name: name,
                    park_id: Some(park_id),
                    build_date: (!build_date().trim().is_empty()).then(|| build_date().trim().to_string()),
                    description: (!description().trim().is_empty()).then(|| description().trim().to_string()),
                    is_own: is_own(),
                };
                error.set(None);
                loading.set(true);
                spawn(async move {
                    let result = if let Some(id) = factory_id {
                        update_factory_record(id, input).await
                    } else {
                        create_factory_with_floors_record(input, Vec::new()).await
                    };
                    match result {
                        Ok(()) => on_done.call(()),
                        Err(message) => { loading.set(false); error.set(Some(message)); }
                    }
                });
            },
            header { class: "section-header", strong { "{title}" } }
            div { class: "form-grid",
                div { class: "field",
                    Label { html_for: "factory-f1", "厂房名称" }
                    Input { id: "factory-f1", value: factory_name(), maxlength: 100, placeholder: "例如：A 栋标准厂房", oninput: move |event: FormEvent| factory_name.set(event.value()) }
                }
                div { class: "field",
                    span { class: "field-label", "建造时间" }
                    DateField { value: build_date(), on_change: move |value: String| build_date.set(value) }
                }
                div { class: "field",
                    Label { html_for: "factory-own", "产权归属" }
                    Select {
                        id: "factory-own",
                        value: Some(own_value),
                        on_value_change: move |value: Option<String>| {
                            if let Some(value) = value { is_own.set(value == "1"); }
                        },
                        SelectOption::<String> { value: "1".to_string(), index: 0usize, text_value: "自有".to_string(), "自有" }
                        SelectOption::<String> { value: "0".to_string(), index: 1usize, text_value: "租入".to_string(), "租入" }
                    }
                }
                div { class: "field is-wide",
                    Label { html_for: "factory-f5", "厂房描述" }
                    Textarea { id: "factory-f5", value: description(), maxlength: 300, placeholder: "补充厂房结构、用途或备注", oninput: move |event: FormEvent| description.set(event.value()) }
                }
            }
            if let Some(message) = error() { p { class: "form-error", "{message}" } }
            footer { class: "form-actions",
                Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_done.call(()), "取消" }
                Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
            }
        }
    }
}

/// 楼层新增与编辑的内联表单，编辑时同时维护楼层图片。
#[component]
pub(super) fn FloorInlineForm(
    factory_id: u64,
    floor: Option<FactoryFloor>,
    previews: Vec<FactoryFloorImagePreview>,
    on_done: EventHandler<()>,
) -> Element {
    let floor_id = floor.as_ref().map(|row| row.floor_id);
    let mut floor_name = use_signal_sync(|| {
        floor
            .as_ref()
            .map(|row| row.floor_name.clone())
            .unwrap_or_default()
    });
    let mut height = use_signal_sync(|| {
        decimal_input(floor.as_ref().and_then(|row| row.floor_height_centi_metres))
    });
    let mut bearing = use_signal_sync(|| {
        decimal_input(floor.as_ref().and_then(|row| row.load_bearing_centi_units))
    });
    let mut rent =
        use_signal_sync(|| decimal_input(floor.as_ref().map(|row| row.rent_price_cents)));
    let mut total = use_signal_sync(|| {
        decimal_input(floor.as_ref().map(|row| row.total_area_centi_square_metres))
    });
    let mut description = use_signal_sync(|| {
        floor
            .as_ref()
            .and_then(|row| row.description.clone())
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

    let title = if floor_id.is_some() {
        "编辑楼层"
    } else {
        "新增楼层"
    };

    rsx! {
        form { class: "inline-form",
            onsubmit: move |event| {
                event.prevent_default();
                if loading() { return; }
                let name = floor_name().trim().to_string();
                if name.is_empty() { error.set(Some("请输入楼层名称".into())); return; }
                let height_value = match parse_decimal(&height(), "层高", true) { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let bearing_value = match parse_decimal(&bearing(), "承重", true) { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let rent_value = match parse_decimal(&rent(), "挂牌租金", false) { Ok(Some(v)) => v, Ok(None) => 0, Err(m) => { error.set(Some(m)); return; } };
                let total_value = match parse_decimal(&total(), "总面积", false) { Ok(Some(v)) => v, Ok(None) => 0, Err(m) => { error.set(Some(m)); return; } };
                let input = FactoryFloorInput {
                    floor_name: name,
                    floor_height_centi_metres: height_value,
                    load_bearing_centi_units: bearing_value,
                    rent_price_cents: rent_value,
                    total_area_centi_square_metres: total_value,
                    description: (!description().trim().is_empty()).then(|| description().trim().to_string()),
                };

                let current = images();
                let existing_image_ids = current.iter().filter_map(|image| match image {
                    BusinessImage::Existing { img_id, removed: false, .. } => Some(*img_id),
                    _ => None,
                }).collect::<Vec<_>>();
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
                            Ok(image) => uploads.push(UploadedFloorImageInput { img_url: image.public_url, hash: image.sha256 }),
                            Err(message) => { loading.set(false); error.set(Some(message)); return; }
                        }
                    }
                    // 新增楼层时先落主档；图片维护在楼层建好后再进入编辑态完成。
                    let result = match floor_id {
                        Some(id) => update_factory_floor_with_images_record(id, input, existing_image_ids, uploads).await,
                        None => create_factory_floor_record(factory_id, input).await,
                    };
                    match result {
                        Ok(()) => {
                            let _ = delete_business_images_from_r2(removed_images).await;
                            discard_image_previews(&images());
                            on_done.call(());
                        }
                        Err(message) => { loading.set(false); error.set(Some(message)); }
                    }
                });
            },
            header { class: "section-header", strong { "{title}" } }
            div { class: "form-grid",
                div { class: "field",
                    Label { html_for: "floor-f1", "楼层名称" }
                    Input { id: "floor-f1", value: floor_name(), maxlength: 50, placeholder: "例如：三楼", oninput: move |event: FormEvent| floor_name.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "floor-f3", "层高（米）" }
                    Input { id: "floor-f3", inputmode: "decimal", value: height(), placeholder: "选填", oninput: move |event: FormEvent| height.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "floor-f4", "承重（吨/㎡）" }
                    Input { id: "floor-f4", inputmode: "decimal", value: bearing(), placeholder: "选填", oninput: move |event: FormEvent| bearing.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "floor-f5", "挂牌租金（元/㎡/月）" }
                    Input { id: "floor-f5", inputmode: "decimal", value: rent(), placeholder: "0.00", oninput: move |event: FormEvent| rent.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "floor-f6", "总面积（㎡）" }
                    Input { id: "floor-f6", inputmode: "decimal", value: total(), placeholder: "0.00", oninput: move |event: FormEvent| total.set(event.value()) }
                }
                div { class: "field is-wide",
                    Label { html_for: "floor-f8", "楼层描述" }
                    Textarea { id: "floor-f8", value: description(), maxlength: 300, placeholder: "补充楼层用途或备注", oninput: move |event: FormEvent| description.set(event.value()) }
                }
            }
            p { class: "hint", "已用面积与出租状态由合同关联算出，不在此填写。" }
            if floor_id.is_some() {
                div { class: "subsection",
                    ImageEditor {
                        images,
                        title: "楼层图片".to_string(),
                        max_count: MAX_FLOOR_IMAGES,
                        loading: loading(),
                        on_error: move |message| error.set(Some(message)),
                    }
                }
            } else {
                p { class: "hint", "楼层创建后即可在编辑中添加图片。" }
            }
            if let Some(message) = error() { p { class: "form-error", "{message}" } }
            footer { class: "form-actions",
                Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_done.call(()), "取消" }
                Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
            }
        }
    }
}

/// 宿舍新增与编辑的内联表单，编辑时同时维护宿舍图片。
#[component]
pub(super) fn DormitoryInlineForm(
    park_id: u64,
    dormitory: Option<Dormitory>,
    previews: Vec<DormitoryImagePreview>,
    on_done: EventHandler<()>,
) -> Element {
    let dormitory_id = dormitory.as_ref().map(|row| row.dormitory_id);
    let mut name = use_signal_sync(|| {
        dormitory
            .as_ref()
            .map(|row| row.dormitory_name.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        dormitory
            .as_ref()
            .and_then(|row| row.remark.clone())
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

    let title = if dormitory_id.is_some() {
        "编辑宿舍"
    } else {
        "新增宿舍"
    };

    rsx! {
        form { class: "inline-form",
            onsubmit: move |event| {
                event.prevent_default();
                if loading() { return; }
                let name_value = name().trim().to_string();
                if name_value.is_empty() { error.set(Some("请输入宿舍名称".into())); return; }
                let input = DormitoryInput {
                    park_id,
                    dormitory_name: name_value,
                    remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                };

                let current = images();
                let existing_image_ids = current.iter().filter_map(|image| match image {
                    BusinessImage::Existing { img_id, removed: false, .. } => Some(*img_id),
                    _ => None,
                }).collect::<Vec<_>>();
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
                            Ok(image) => uploads.push(UploadedDormitoryImageInput { img_url: image.public_url, hash: image.sha256 }),
                            Err(message) => { loading.set(false); error.set(Some(message)); return; }
                        }
                    }
                    let result = match dormitory_id {
                        Some(id) => update_dormitory_with_images_record(id, input, existing_image_ids, uploads).await,
                        None => create_dormitory_record(input).await,
                    };
                    match result {
                        Ok(()) => {
                            let _ = delete_business_images_from_r2(removed_images).await;
                            discard_image_previews(&images());
                            on_done.call(());
                        }
                        Err(message) => { loading.set(false); error.set(Some(message)); }
                    }
                });
            },
            header { class: "section-header", strong { "{title}" } }
            div { class: "form-grid",
                div { class: "field",
                    Label { html_for: "dormitory-f1", "宿舍名称" }
                    Input { id: "dormitory-f1", value: name(), maxlength: 100, placeholder: "例如：一号员工宿舍", oninput: move |event: FormEvent| name.set(event.value()) }
                }
                p { class: "hint is-wide",
                    "楼层数、房间数、层高和挂牌租金改为逐层维护；已用房间数由合同关联算出，不再手工填写。"
                }
                div { class: "field is-wide",
                    Label { html_for: "dormitory-f11", "宿舍备注" }
                    Textarea { id: "dormitory-f11", value: remark(), maxlength: 300, placeholder: "补充宿舍配套或管理说明", oninput: move |event: FormEvent| remark.set(event.value()) }
                }
            }
            if dormitory_id.is_some() {
                div { class: "subsection",
                    ImageEditor {
                        images,
                        title: "宿舍图片".to_string(),
                        max_count: MAX_DORMITORY_IMAGES,
                        loading: loading(),
                        on_error: move |message| error.set(Some(message)),
                    }
                }
            } else {
                p { class: "hint", "宿舍创建后即可在编辑中添加图片。" }
            }
            if let Some(message) = error() { p { class: "form-error", "{message}" } }
            footer { class: "form-actions",
                Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_done.call(()), "取消" }
                Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
            }
        }
    }
}

/// 宿舍楼层的新增与编辑表单。
///
/// 只填这一层"是什么"：房间数、面积、层高、挂牌租金。占用了几间不在这里
/// 填——那由合同关联算出来。
#[component]
pub(super) fn DormitoryFloorInlineForm(
    dormitory_id: u64,
    floor: Option<DormitoryFloor>,
    on_done: EventHandler<()>,
) -> Element {
    let dormitory_floor_id = floor.as_ref().map(|row| row.dormitory_floor_id);
    let mut floor_no = use_signal_sync(|| {
        floor
            .as_ref()
            .map(|row| row.floor_no.to_string())
            .unwrap_or_else(|| "1".to_string())
    });
    let mut room_count = use_signal_sync(|| {
        floor
            .as_ref()
            .map(|row| row.room_count.to_string())
            .unwrap_or_else(|| "0".to_string())
    });
    let mut room_area = use_signal_sync(|| {
        decimal_input(floor.as_ref().and_then(|row| row.room_area_centi_square_metres))
    });
    let mut height = use_signal_sync(|| {
        decimal_input(floor.as_ref().and_then(|row| row.floor_height_centi_metres))
    });
    let mut rent = use_signal_sync(|| {
        decimal_input(floor.as_ref().and_then(|row| row.rent_price_cents))
    });
    let mut remark = use_signal_sync(|| {
        floor
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);

    let title = if dormitory_floor_id.is_some() {
        "编辑宿舍楼层"
    } else {
        "新增宿舍楼层"
    };

    rsx! {
        form { class: "inline-form",
            onsubmit: move |event| {
                event.prevent_default();
                if loading() { return; }
                let floor_no_value = match parse_count(&floor_no(), "楼层号") { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                if floor_no_value <= 0 { error.set(Some("楼层号必须大于零".into())); return; }
                let room_count_value = match parse_count(&room_count(), "房间数") { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let room_area_value = match parse_decimal(&room_area(), "单间面积", true) { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let height_value = match parse_decimal(&height(), "层高", true) { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let rent_value = match parse_decimal(&rent(), "挂牌租金", true) { Ok(v) => v, Err(m) => { error.set(Some(m)); return; } };
                let input = DormitoryFloorInput {
                    dormitory_id,
                    floor_no: floor_no_value,
                    room_count: room_count_value,
                    room_area_centi_square_metres: room_area_value,
                    floor_height_centi_metres: height_value,
                    rent_price_cents: rent_value,
                    remark: (!remark().trim().is_empty()).then(|| remark().trim().to_string()),
                };
                error.set(None);
                loading.set(true);
                spawn(async move {
                    let result = match dormitory_floor_id {
                        Some(id) => update_dormitory_floor_record(id, input).await,
                        None => create_dormitory_floor_record(input).await,
                    };
                    loading.set(false);
                    match result {
                        Ok(()) => on_done.call(()),
                        Err(message) => error.set(Some(message)),
                    }
                });
            },
            h4 { "{title}" }
            div { class: "form-grid",
                div { class: "field",
                    Label { html_for: "dorm-floor-f1", "第几层" }
                    Input { id: "dorm-floor-f1", inputmode: "numeric", value: floor_no(), oninput: move |event: FormEvent| floor_no.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "dorm-floor-f2", "房间数" }
                    Input { id: "dorm-floor-f2", inputmode: "numeric", value: room_count(), oninput: move |event: FormEvent| room_count.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "dorm-floor-f3", "单间面积（㎡）" }
                    Input { id: "dorm-floor-f3", inputmode: "decimal", value: room_area(), placeholder: "选填", oninput: move |event: FormEvent| room_area.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "dorm-floor-f4", "层高（米）" }
                    Input { id: "dorm-floor-f4", inputmode: "decimal", value: height(), placeholder: "选填", oninput: move |event: FormEvent| height.set(event.value()) }
                }
                div { class: "field",
                    Label { html_for: "dorm-floor-f5", "挂牌租金（元/间/月）" }
                    Input { id: "dorm-floor-f5", inputmode: "decimal", value: rent(), placeholder: "选填", oninput: move |event: FormEvent| rent.set(event.value()) }
                }
                div { class: "field is-wide",
                    Label { html_for: "dorm-floor-f6", "楼层备注" }
                    Textarea { id: "dorm-floor-f6", value: remark(), maxlength: 300, placeholder: "补充该层用途或说明", oninput: move |event: FormEvent| remark.set(event.value()) }
                }
            }
            p { class: "hint", "已用房间数由合同关联算出，不在此填写。" }
            if let Some(message) = error() { p { class: "form-error", "{message}" } }
            footer { class: "form-actions",
                Button { variant: ButtonVariant::Outline, r#type: "button", disabled: loading(), onclick: move |_| on_done.call(()), "取消" }
                Button { r#type: "submit", disabled: loading(), if loading() { "保存中…" } else { "确认保存" } }
            }
        }
    }
}
