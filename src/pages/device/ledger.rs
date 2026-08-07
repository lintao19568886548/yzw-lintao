//! 设备台账页：摄像头与门禁设备共用一套列表与表单。
//!
//! 两页只差「可登记哪几种类型」，其余字段、校验、权限形状完全一样，所以是同一个
//! 组件按分类参数化，而不是两份互相漂移的复制品——参照智能水电表管理页对电表与
//! 水表的处理。设计见 `docs/设备管理.md`。

use dioxus::prelude::*;

use super::model::*;
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent},
        dialog::{Dialog, DialogDescription, DialogTitle},
        discard_image_previews,
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        textarea::Textarea,
        total_pages, BusinessImage, ConfirmDialog, DateField, ImageEditor, Pager,
    },
    services::{
        delete_business_images_from_r2, delete_device_asset_record, save_device_asset_record,
        upload_business_image, StoredR2Image,
    },
    spacetime_bindings::{
        device_asset_image_preview_type::DeviceAssetImagePreview,
        device_asset_input_type::DeviceAssetInput, device_asset_type::DeviceAsset,
        factory_type::Factory, park_type::Park,
        uploaded_device_image_input_type::UploadedDeviceImageInput,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

/// 与服务端 `MAX_DEVICE_IMAGES` 保持一致。
const MAX_IMAGES: usize = 8;

#[component]
pub fn DeviceCameraPage() -> Element {
    rsx! { DeviceLedgerPage { device_class: CLASS_SENSOR.to_string() } }
}

#[component]
pub fn DeviceAccessPage() -> Element {
    rsx! { DeviceLedgerPage { device_class: CLASS_ACTUATOR.to_string() } }
}

/// 本页可登记的设备类型。
fn types_for(device_class: &str) -> &'static [&'static str] {
    if device_class == CLASS_SENSOR {
        &SENSOR_TYPES
    } else {
        &ACTUATOR_TYPES
    }
}

fn page_title(device_class: &str) -> &'static str {
    if device_class == CLASS_SENSOR {
        "摄像头管理"
    } else {
        "门禁设备管理"
    }
}

fn page_subtitle(device_class: &str) -> &'static str {
    if device_class == CLASS_SENSOR {
        "摄像头属于传感器：故障时影响的是画面采集，不影响人员通行。台账记录安装位置及其对应的平台通道，视频流不进入本系统。"
    } else {
        "门禁、道闸与闸机属于执行终端：故障时将直接影响人员与车辆通行。本页管理设备本体；通行记录见门禁管理。"
    }
}

#[component]
fn DeviceLedgerPage(device_class: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let class_key = device_class.clone();
    let mut query = use_signal(String::new);
    let mut type_filter = use_signal(|| "全部".to_string());
    let mut page = use_signal(|| 1usize);
    let mut feedback = use_signal_sync(|| None::<String>);
    // Some(None) = 新增，Some(Some(asset)) = 编辑。
    let mut asset_form = use_signal(|| None::<Option<DeviceAsset>>);
    let mut confirm_delete = use_signal(|| None::<DeviceAsset>);
    let deleting = use_signal_sync(|| false);

    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let types = types_for(&class_key);

    let all_rows = state
        .device_assets
        .read()
        .iter()
        .filter(|row| row.device_class == class_key)
        .cloned()
        .collect::<Vec<_>>();
    let total_assets = all_rows.len();
    let bound = all_rows
        .iter()
        .filter(|row| {
            row.external_device_id
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        })
        .count();
    let unbound = total_assets - bound;

    let query_value = query().trim().to_lowercase();
    let type_value = type_filter();
    let rows = all_rows
        .into_iter()
        .filter(|row| {
            (type_value == "全部" || device_type_label(&row.device_type) == type_value)
                && (query_value.is_empty()
                    || format!(
                        "{} {} {} {} {} {}",
                        row.device_name,
                        row.device_code,
                        row.location,
                        row.external_device_id.clone().unwrap_or_default(),
                        park_label(&parks, row.park_id),
                        factory_label(&factories, row.factory_id),
                    )
                    .to_lowercase()
                    .contains(&query_value))
        })
        .collect::<Vec<_>>();
    let total = rows.len();
    let page_count = total_pages(total, PAGE_SIZE);
    let visible = rows
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();
    let type_value_signal: ReadSignal<Option<String>> =
        use_memo(move || Some(type_filter())).into();
    let title = page_title(&class_key);

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "{title}" }
                    p { class: "page-subtitle", "{page_subtitle(&class_key)}" }
                }
                div { class: "page-actions",
                    Button {
                        r#type: "button",
                        onclick: move |_| {
                            feedback.set(None);
                            asset_form.set(Some(None));
                        },
                        "登记设备"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3", aria_label: "设备台账总览",
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "在册设备" }
                    strong { class: "stat-value is-mono", "{total_assets}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "已填平台设备号" }
                    strong { class: "stat-value is-mono", "{bound}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "未接入平台" }
                    strong { class: "stat-value is-mono", "{unbound}" }
                } } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "filters",
                    div { class: "field",
                        Label { html_for: "device-query", "搜索" }
                        Input {
                            id: "device-query",
                            value: query,
                            placeholder: "名称、编号、位置、平台设备号、园区或厂房",
                            oninput: move |event: FormEvent| {
                                query.set(event.value());
                                page.set(1);
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-type-filter", "设备类型" }
                        Select {
                            id: "device-type-filter",
                            value: Some(type_value_signal),
                            on_value_change: move |value: Option<String>| {
                                type_filter.set(value.unwrap_or_else(|| "全部".into()));
                                page.set(1);
                            },
                            SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                            for (index , device_type) in types.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "device-type-{device_type}",
                                    value: device_type_label(device_type).to_string(),
                                    index: index + 1,
                                    text_value: device_type_label(device_type).to_string(),
                                    "{device_type_label(device_type)}"
                                }
                            }
                        }
                    }
                } } }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "{title}台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 台" }
                }
                if visible.is_empty() {
                    p { class: "empty",
                        "暂无符合条件的设备。请先登记设备的安装位置与编号；待厂商平台接口接入后，已填写平台设备号的设备方可获取在线状态。"
                    }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "设备" }
                                    th { "位置" }
                                    th { "类型 / 型号" }
                                    th { "平台接入" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in visible {
                                    tr { key: "{row.asset_id}",
                                        td {
                                            div { class: "stack-tight",
                                                strong { "{row.device_name}" }
                                                small { class: "hint is-mono", "{row.device_code}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { "{park_label(&parks, row.park_id)} · {factory_label(&factories, row.factory_id)}" }
                                                small { class: "hint", "{row.location}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { "{device_type_label(&row.device_type)}" }
                                                small { class: "hint",
                                                    {
                                                        let vendor = row.vendor.clone().unwrap_or_default();
                                                        let model = row.model.clone().unwrap_or_default();
                                                        let text = format!("{vendor} {model}");
                                                        if text.trim().is_empty() { "厂商与型号未填写".to_string() } else { text.trim().to_string() }
                                                    }
                                                }
                                            }
                                        }
                                        td {
                                            match row.external_device_id.as_deref() {
                                                Some(value) if !value.trim().is_empty() => rsx! {
                                                    div { class: "stack-tight",
                                                        Badge { variant: BadgeVariant::Secondary, "已填设备号" }
                                                        small { class: "hint is-mono", "{value}" }
                                                    }
                                                },
                                                _ => rsx! {
                                                    div { class: "stack-tight",
                                                        Badge { variant: BadgeVariant::Outline, "未接入" }
                                                        small { class: "hint", "当前仅为静态台账记录" }
                                                    }
                                                },
                                            }
                                        }
                                        td {
                                            div { class: "table-actions",
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| {
                                                            feedback.set(None);
                                                            asset_form.set(Some(Some(row.clone())));
                                                        }
                                                    },
                                                    "编辑"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    class: "is-quiet-danger",
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let row = row.clone();
                                                        move |_| confirm_delete.set(Some(row.clone()))
                                                    },
                                                    "注销"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Pager { page, total_pages: page_count, total_count: total }
                }
            }
        }

        if let Some(editing) = asset_form() {
            DeviceAssetDialog {
                device_class: class_key.clone(),
                asset: editing.clone(),
                previews: editing
                    .as_ref()
                    .map(|asset| {
                        state
                            .device_asset_image_previews
                            .read()
                            .iter()
                            .filter(|preview| preview.asset_id == asset.asset_id)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                parks: parks.clone(),
                factories: factories.clone(),
                on_close: move |_| asset_form.set(None),
                on_saved: move |_| {
                    asset_form.set(None);
                    feedback.set(Some("保存成功，台账正在实时更新".into()));
                },
            }
        }

        if let Some(asset) = confirm_delete() {
            ConfirmDialog {
                title: "确认注销这台设备？",
                description: "此操作为逻辑删除：设备将从台账中隐藏，数据不会被物理删除。",
                confirm_label: if deleting() { "注销中…" } else { "确认注销" },
                on_cancel: move |_| confirm_delete.set(None),
                on_confirm: move |_| {
                    if deleting() {
                        return;
                    }
                    let asset_id = asset.asset_id;
                    let mut feedback = feedback;
                    let mut confirm_delete = confirm_delete;
                    let mut deleting = deleting;
                    deleting.set(true);
                    spawn(async move {
                        let result = delete_device_asset_record(asset_id).await;
                        deleting.set(false);
                        confirm_delete.set(None);
                        feedback.set(Some(match result {
                            Ok(()) => "设备已注销".into(),
                            Err(error) => error,
                        }));
                    });
                },
            }
        }
    }
}

#[component]
fn DeviceAssetDialog(
    device_class: String,
    asset: Option<DeviceAsset>,
    previews: Vec<DeviceAssetImagePreview>,
    parks: Vec<Park>,
    factories: Vec<Factory>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let asset_id = asset.as_ref().map(|row| row.asset_id);
    let types = types_for(&device_class);
    let mut device_type = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.device_type.clone())
            .unwrap_or_else(|| types[0].to_string())
    });
    let mut name = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.device_name.clone())
            .unwrap_or_default()
    });
    let mut code = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.device_code.clone())
            .unwrap_or_default()
    });
    let mut location = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.location.clone())
            .unwrap_or_default()
    });
    let mut vendor = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.vendor.clone())
            .unwrap_or_default()
    });
    let mut model = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.model.clone())
            .unwrap_or_default()
    });
    let mut external_device_id = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.external_device_id.clone())
            .unwrap_or_default()
    });
    let mut commissioned_on = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.commissioned_on.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    let mut park_id = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.park_id.to_string())
            .unwrap_or_default()
    });
    let mut factory_id = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.factory_id.to_string())
            .unwrap_or_else(|| "0".into())
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

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();
    let factory_value: ReadSignal<Option<String>> = use_memo(move || Some(factory_id())).into();
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(device_type())).into();
    let selected_park = park_id().parse::<u64>().ok();
    let park_factories = factories
        .iter()
        .filter(|row| selected_park == Some(row.park_id))
        .cloned()
        .collect::<Vec<_>>();

    rsx! {
        Dialog {
            open: Some(true),
            is_modal: true,
            on_open_change: move |open: bool| {
                if !open && !loading() {
                    on_close.call(());
                }
            },
            DialogTitle { if asset_id.is_some() { "编辑设备" } else { "登记设备" } }
            DialogDescription { "设备信息与现场照片将一并保存。设备分类由设备类型自动判定，无需单独选择。" }
            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "device-park", "所属园区 *" }
                        Select {
                            id: "device-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                park_id.set(value.unwrap_or_default());
                                // 换园区后原厂房不再属于这个园区，回落到公共区域。
                                factory_id.set("0".into());
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "device-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-factory", "所在厂房" }
                        Select {
                            id: "device-factory",
                            value: Some(factory_value),
                            on_value_change: move |value: Option<String>| {
                                factory_id.set(value.unwrap_or_else(|| "0".into()))
                            },
                            SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "园区公共区域".to_string(), "园区公共区域" }
                            for (index , factory) in park_factories.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "device-factory-{factory.factory_id}",
                                    value: factory.factory_id.to_string(),
                                    index: index + 1,
                                    text_value: factory.factory_name.to_string(),
                                    "{factory.factory_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-type", "设备类型 *" }
                        Select {
                            id: "device-type",
                            value: Some(type_value),
                            on_value_change: move |value: Option<String>| {
                                if let Some(value) = value {
                                    device_type.set(value);
                                }
                            },
                            for (index , option) in types.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "device-type-option-{option}",
                                    value: (*option).to_string(),
                                    index,
                                    text_value: device_type_label(option).to_string(),
                                    "{device_type_label(option)}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-code", "设备编号 *" }
                        Input {
                            id: "device-code",
                            value: code(),
                            maxlength: 50,
                            placeholder: "园区内唯一，与现场标签一致",
                            oninput: move |event: FormEvent| code.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-name", "设备名称 *" }
                        Input {
                            id: "device-name",
                            value: name(),
                            maxlength: 100,
                            placeholder: "例如：北大门 1 号枪机",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "device-location", "位置描述 *" }
                        Input {
                            id: "device-location",
                            value: location(),
                            maxlength: 200,
                            placeholder: "例如：北大门东侧立杆",
                            oninput: move |event: FormEvent| location.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-vendor", "厂商" }
                        Input {
                            id: "device-vendor",
                            value: vendor(),
                            maxlength: 50,
                            placeholder: "例如：海康",
                            oninput: move |event: FormEvent| vendor.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "device-model", "型号" }
                        Input {
                            id: "device-model",
                            value: model(),
                            maxlength: 50,
                            placeholder: "报修与更换配件时使用",
                            oninput: move |event: FormEvent| model.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "device-external", "平台设备号" }
                        Input {
                            id: "device-external",
                            value: external_device_id(),
                            maxlength: 100,
                            placeholder: "厂商平台上的设备编号，暂无可留空",
                            oninput: move |event: FormEvent| external_device_id.set(event.value()),
                        }
                        small { class: "hint",
                            "留空不影响台账登记，仅无法获取在线状态。厂商平台接口尚未接入，此处可先行记录设备号。"
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "启用日期" }
                        DateField {
                            value: commissioned_on(),
                            on_change: move |value: String| commissioned_on.set(value),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "device-remark", "备注" }
                        Textarea {
                            id: "device-remark",
                            maxlength: 200,
                            rows: 2,
                            value: remark(),
                            placeholder: "可补充安装单位、维保联系人等信息",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                }
                div { class: "field is-wide",
                    ImageEditor {
                        images,
                        title: "现场照片".to_string(),
                        max_count: MAX_IMAGES,
                        loading: loading(),
                        on_error: move |message| error.set(Some(message)),
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
                    Button {
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| {
                            if loading() {
                                return;
                            }
                            let park_id = match park_id().parse::<u64>() {
                                Ok(value) if value != 0 => value,
                                _ => {
                                    error.set(Some("请选择所属园区".into()));
                                    return;
                                }
                            };
                            let factory_id = factory_id().parse::<u64>().unwrap_or(0);
                            let name_value = name().trim().to_string();
                            if name_value.is_empty() {
                                error.set(Some("请输入设备名称".into()));
                                return;
                            }
                            let code_value = code().trim().to_string();
                            if code_value.is_empty() {
                                error.set(Some("请输入设备编号".into()));
                                return;
                            }
                            let location_value = location().trim().to_string();
                            if location_value.is_empty() {
                                error.set(Some("请输入位置描述".into()));
                                return;
                            }
                            let input = DeviceAssetInput {
                                device_type: device_type(),
                                device_name: name_value,
                                device_code: code_value,
                                location: location_value,
                                vendor: (!vendor().trim().is_empty())
                                    .then(|| vendor().trim().to_string()),
                                model: (!model().trim().is_empty())
                                    .then(|| model().trim().to_string()),
                                external_device_id: (!external_device_id().trim().is_empty())
                                    .then(|| external_device_id().trim().to_string()),
                                commissioned_on: (!commissioned_on().trim().is_empty())
                                    .then(|| commissioned_on().trim().to_string()),
                                remark: (!remark().trim().is_empty())
                                    .then(|| remark().trim().to_string()),
                                factory_id,
                                park_id,
                            };
                            let current = images();
                            let existing_image_ids = current
                                .iter()
                                .filter_map(|image| match image {
                                    BusinessImage::Existing { img_id, removed: false, .. } => Some(*img_id),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let removed_images = current
                                .iter()
                                .filter_map(|image| match image {
                                    BusinessImage::Existing { img_id, url, removed: true } => {
                                        Some(StoredR2Image { img_id: *img_id, public_url: url.clone() })
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let pending_files = current
                                .into_iter()
                                .filter_map(|image| match image {
                                    BusinessImage::Pending { file, .. } => Some(file),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            error.set(None);
                            loading.set(true);
                            let images = images;
                            spawn(async move {
                                let mut uploads = Vec::with_capacity(pending_files.len());
                                for file in pending_files {
                                    match upload_business_image(file).await {
                                        Ok(image) => uploads.push(UploadedDeviceImageInput {
                                            img_url: image.public_url,
                                            hash: image.sha256,
                                        }),
                                        Err(message) => {
                                            loading.set(false);
                                            error.set(Some(message));
                                            return;
                                        }
                                    }
                                }
                                let saved = save_device_asset_record(
                                    asset_id,
                                    input,
                                    existing_image_ids,
                                    uploads,
                                )
                                .await;
                                match saved {
                                    Ok(()) => {
                                        let _ = delete_business_images_from_r2(removed_images).await;
                                        discard_image_previews(&images());
                                        loading.set(false);
                                        on_saved.call(());
                                    }
                                    Err(message) => {
                                        loading.set(false);
                                        error.set(Some(message));
                                    }
                                }
                            });
                        },
                        if loading() { "保存中…" } else { "确认保存" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 摄像头页只放传感器类型门禁页只放执行终端类型() {
        assert_eq!(types_for(CLASS_SENSOR), &SENSOR_TYPES);
        assert_eq!(types_for(CLASS_ACTUATOR), &ACTUATOR_TYPES);
        // 两页的类型集合不能重叠，否则同一台设备能从两个入口登记，
        // 权限也就形同虚设。
        for sensor in SENSOR_TYPES {
            assert!(!ACTUATOR_TYPES.contains(&sensor), "{sensor} 出现在两边");
        }
    }

    #[test]
    fn 两页标题不同() {
        assert_eq!(page_title(CLASS_SENSOR), "摄像头管理");
        assert_eq!(page_title(CLASS_ACTUATOR), "门禁设备管理");
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn CameraRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { DeviceCameraPage {} }
    }

    #[component]
    fn AccessRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { DeviceAccessPage {} }
    }

    #[test]
    fn 摄像头管理页可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { CameraRenderTestRoot {} });
        assert!(html.contains("摄像头管理"), "页面标题没有进入渲染结果");
    }

    #[test]
    fn 门禁设备管理页可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { AccessRenderTestRoot {} });
        assert!(html.contains("门禁设备管理"), "页面标题没有进入渲染结果");
    }
}
