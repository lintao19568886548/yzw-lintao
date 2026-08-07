//! 电梯资产台账页：一行一台真实电梯。
//!
//! 与变压器台账（`transformer.rs`）同构，差异只有一条：电梯必须属于某个
//! 厂房——表单只选厂房不选园区，园区由服务端从厂房推导。
//! 设计见 `docs/电梯台账与扫码巡检.md`。

use dioxus::prelude::*;

use super::model::*;
use super::transformer::{can_submit_inspection, inspect_url, print_qr_sheet, qr_svg, QrPrintLabel};
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
        total_pages, BusinessImage, ConfirmDialog, DateField, ImageEditor, Pager, TimeField,
    },
    services::{
        create_elevator_inspection_record, delete_business_images_from_r2,
        delete_elevator_asset_record, save_elevator_asset_record, upload_business_image,
        StoredR2Image,
    },
    spacetime_bindings::{
        elevator_asset_image_preview_type::ElevatorAssetImagePreview,
        elevator_asset_input_type::ElevatorAssetInput, elevator_asset_type::ElevatorAsset,
        elevator_inspection_image_preview_type::ElevatorInspectionImagePreview,
        elevator_inspection_input_type::ElevatorInspectionInput,
        elevator_inspection_type::ElevatorInspection, factory_type::Factory, park_type::Park,
        uploaded_maintenance_image_input_type::UploadedMaintenanceImageInput,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

/// 与服务端 `MAX_MAINTENANCE_IMAGES` 保持一致。
const MAX_IMAGES: usize = 8;

#[derive(Clone, PartialEq)]
struct AssetRow {
    asset: ElevatorAsset,
    latest: Option<ElevatorInspection>,
}

impl AssetRow {
    fn status_label(&self) -> &str {
        self.latest
            .as_ref()
            .map(|row| row.status.as_str())
            .unwrap_or("未巡检")
    }
}

#[component]
/// `factory` 来自 URL 查询参数：从园区档案页的设施概览卡跳来时预填搜索框，
/// 落地即是该厂房的设备（园区管理.md §2.3.1）。直接打开本页时为空串。
pub fn MaintenanceElevatorPage(factory: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut query = use_signal(|| factory.clone());
    let mut status_filter = use_signal(|| "全部".to_string());
    let mut page = use_signal(|| 1usize);
    let mut feedback = use_signal_sync(|| None::<String>);
    // Some(None) = 新增，Some(Some(asset)) = 编辑。
    let mut asset_form = use_signal(|| None::<Option<ElevatorAsset>>);
    let mut inspect_target = use_signal(|| None::<ElevatorAsset>);
    let mut history_target = use_signal(|| None::<ElevatorAsset>);
    let mut qr_target = use_signal(|| None::<ElevatorAsset>);
    let mut confirm_delete = use_signal(|| None::<ElevatorAsset>);
    let mut batch_print = use_signal(|| false);
    let deleting = use_signal_sync(|| false);

    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let inspections = state.elevator_inspections.read().clone();
    let can_inspect = can_submit_inspection(&state);

    let all_rows = state
        .elevator_assets
        .read()
        .iter()
        .map(|asset| AssetRow {
            latest: latest_elevator_inspection(&inspections, asset.asset_id),
            asset: asset.clone(),
        })
        .collect::<Vec<_>>();
    let total_assets = all_rows.len();
    let abnormal = all_rows
        .iter()
        .filter(|row| row.status_label() == "异常")
        .count();
    let never = all_rows
        .iter()
        .filter(|row| row.latest.is_none())
        .count();

    let query_value = query().trim().to_lowercase();
    let status_value = status_filter();
    let rows = all_rows
        .into_iter()
        .filter(|row| {
            (status_value == "全部" || row.status_label() == status_value)
                && (query_value.is_empty()
                    || format!(
                        "{} {} {} {} {}",
                        row.asset.elevator_name,
                        row.asset.location,
                        row.asset.size.clone().unwrap_or_default(),
                        park_name(&parks, row.asset.park_id),
                        factory_name(&factories, Some(row.asset.factory_id)),
                    )
                    .to_lowercase()
                    .contains(&query_value))
        })
        .collect::<Vec<_>>();
    let print_rows = rows.clone();
    let total = rows.len();
    let page_count = total_pages(total, PAGE_SIZE);
    let visible = rows
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();
    let status_value_signal: ReadSignal<Option<String>> =
        use_memo(move || Some(status_filter())).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "电梯管理" }
                    p { class: "page-subtitle", "一行一台电梯，必须挂在具体厂房。运行状态来自最近一次巡检。" }
                }
                div { class: "page-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: total_assets == 0,
                        onclick: move |_| batch_print.set(true),
                        "打印巡检码"
                    }
                    Button {
                        r#type: "button",
                        onclick: move |_| {
                            feedback.set(None);
                            asset_form.set(Some(None));
                        },
                        "新增电梯"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3", aria_label: "电梯台账总览",
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "在册设备" }
                    strong { class: "stat-value is-mono", "{total_assets}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "最近巡检异常" }
                    strong { class: "stat-value is-mono is-bad", "{abnormal}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "从未巡检" }
                    strong { class: "stat-value is-mono", "{never}" }
                } } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "filters",
                    div { class: "field",
                        Label { html_for: "elevator-query", "搜索" }
                        Input {
                            id: "elevator-query",
                            value: query,
                            placeholder: "名称、位置、尺寸、园区或厂房",
                            oninput: move |event: FormEvent| {
                                query.set(event.value());
                                page.set(1);
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "elevator-status", "最近巡检状态" }
                        Select {
                            id: "elevator-status",
                            value: Some(status_value_signal),
                            on_value_change: move |value: Option<String>| {
                                status_filter.set(value.unwrap_or_else(|| "全部".into()));
                                page.set(1);
                            },
                            SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                            for (index , value) in ["正常", "异常", "未巡检"].into_iter().enumerate() {
                                SelectOption::<String> {
                                    key: "elevator-status-{value}",
                                    value: value.to_string(),
                                    index: index + 1,
                                    text_value: value.to_string(),
                                    "{value}"
                                }
                            }
                        }
                    }
                } } }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "电梯管理台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 台" }
                }
                if visible.is_empty() {
                    p { class: "empty", "暂无符合条件的设备。先在这里录入电梯资产，再打印巡检码贴到设备上。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "设备" }
                                    th { "位置" }
                                    th { "尺寸 / 承重" }
                                    th { "最近巡检" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in visible {
                                    tr { key: "{row.asset.asset_id}",
                                        td {
                                            div { class: "stack-tight",
                                                strong { "{row.asset.elevator_name}" }
                                                small { class: "hint is-mono", "LIFT-{row.asset.asset_id:06}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { "{park_name(&parks, row.asset.park_id)} · {factory_name(&factories, Some(row.asset.factory_id))}" }
                                                small { class: "hint", "{row.asset.location}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { {row.asset.size.clone().unwrap_or_else(|| "尺寸未记录".into())} }
                                                small { class: "hint", "{format_load_kg(row.asset.load_capacity_centi_kg)}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                Badge { variant: status_variant(row.status_label()), "{row.status_label()}" }
                                                if let Some(latest) = &row.latest {
                                                    small { class: "hint", "{format_datetime(latest.check_time)} · {latest.inspector_name}" }
                                                } else {
                                                    small { class: "hint", "尚无巡检记录" }
                                                }
                                            }
                                        }
                                        td {
                                            div { class: "table-actions",
                                                if can_inspect {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Sm,
                                                        r#type: "button",
                                                        onclick: {
                                                            let asset = row.asset.clone();
                                                            move |_| inspect_target.set(Some(asset.clone()))
                                                        },
                                                        "登记巡检"
                                                    }
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let asset = row.asset.clone();
                                                        move |_| history_target.set(Some(asset.clone()))
                                                    },
                                                    "巡检历史"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let asset = row.asset.clone();
                                                        move |_| qr_target.set(Some(asset.clone()))
                                                    },
                                                    "巡检码"
                                                }
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    size: ButtonSize::Sm,
                                                    r#type: "button",
                                                    onclick: {
                                                        let asset = row.asset.clone();
                                                        move |_| {
                                                            feedback.set(None);
                                                            asset_form.set(Some(Some(asset.clone())));
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
                                                        let asset = row.asset.clone();
                                                        move |_| confirm_delete.set(Some(asset.clone()))
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
            ElevatorAssetDialog {
                asset: editing.clone(),
                previews: editing
                    .as_ref()
                    .map(|asset| {
                        state
                            .elevator_asset_image_previews
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

        if let Some(asset) = inspect_target() {
            Dialog {
                open: Some(true),
                on_open_change: move |open: bool| {
                    if !open {
                        inspect_target.set(None);
                    }
                },
                DialogTitle { "登记巡检 · {asset.elevator_name}" }
                DialogDescription { "巡检记录只增不删，填错请补一条更正记录。" }
                ElevatorInspectionForm {
                    asset: asset.clone(),
                    on_saved: move |_| {
                        inspect_target.set(None);
                        feedback.set(Some("巡检已提交".into()));
                    },
                    on_cancel: move |_| inspect_target.set(None),
                }
            }
        }

        if let Some(asset) = history_target() {
            ElevatorHistoryDialog {
                asset: asset.clone(),
                inspections: inspections
                    .iter()
                    .filter(|row| row.asset_id == asset.asset_id)
                    .cloned()
                    .collect::<Vec<_>>(),
                previews: state.elevator_inspection_image_previews.read().clone(),
                on_close: move |_| history_target.set(None),
            }
        }

        if let Some(asset) = qr_target() {
            ElevatorQrDialog { asset, on_close: move |_| qr_target.set(None) }
        }

        if batch_print() {
            Dialog {
                open: Some(true),
                on_open_change: move |open: bool| {
                    if !open {
                        batch_print.set(false);
                    }
                },
                DialogTitle { "打印巡检码" }
                DialogDescription { "共 {print_rows.len()} 台电梯（按当前筛选）。打印时只输出标签区域，裁开后贴到设备上。" }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| batch_print.set(false),
                        "取消"
                    }
                    Button { r#type: "button", onclick: move |_| print_qr_sheet(), "打印" }
                }
                div { class: "qr-print-sheet",
                    div { class: "qr-label-grid",
                        for row in print_rows.iter() {
                            QrPrintLabel {
                                key: "{row.asset.asset_id}",
                                title: row.asset.elevator_name.clone(),
                                code: format!("LIFT-{:06}", row.asset.asset_id),
                                location: row.asset.location.clone(),
                                url: inspect_url("elevator", row.asset.asset_id),
                            }
                        }
                    }
                }
            }
        }

        if let Some(asset) = confirm_delete() {
            ConfirmDialog {
                title: "确认注销这台电梯？",
                description: "这是逻辑删除：设备与巡检历史从台账隐藏，数据不会被真正删除。",
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
                        let result = delete_elevator_asset_record(asset_id).await;
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
fn ElevatorAssetDialog(
    asset: Option<ElevatorAsset>,
    previews: Vec<ElevatorAssetImagePreview>,
    parks: Vec<Park>,
    factories: Vec<Factory>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let asset_id = asset.as_ref().map(|row| row.asset_id);
    let mut name = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.elevator_name.clone())
            .unwrap_or_default()
    });
    let mut location = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.location.clone())
            .unwrap_or_default()
    });
    let mut size = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.size.clone())
            .unwrap_or_default()
    });
    let mut load_capacity = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| load_kg_input(row.load_capacity_centi_kg))
            .unwrap_or_default()
    });
    let mut production_date = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.production_date.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.remark.clone())
            .unwrap_or_default()
    });
    // 园区下拉只用来筛选厂房，提交的只有厂房；园区由服务端从厂房推导。
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

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();
    let factory_value: ReadSignal<Option<String>> = use_memo(move || Some(factory_id())).into();
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
            DialogTitle { if asset_id.is_some() { "编辑电梯" } else { "新增电梯" } }
            DialogDescription { "电梯必须挂在具体厂房，园区随厂房自动确定；运行状态由巡检记录得出。" }
            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "elevator-park", "所属园区（筛选厂房用）" }
                        Select {
                            id: "elevator-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                park_id.set(value.unwrap_or_default());
                                factory_id.set(String::new());
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "elevator-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "elevator-factory", "所在厂房 *" }
                        Select {
                            id: "elevator-factory",
                            value: Some(factory_value),
                            on_value_change: move |value: Option<String>| {
                                factory_id.set(value.unwrap_or_default())
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择厂房".to_string(), "请选择厂房" }
                            for (index , factory) in park_factories.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "elevator-factory-{factory.factory_id}",
                                    value: factory.factory_id.to_string(),
                                    index: index + 1,
                                    text_value: factory.factory_name.to_string(),
                                    "{factory.factory_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "elevator-name", "设备名称 *" }
                        Input {
                            id: "elevator-name",
                            value: name(),
                            maxlength: 100,
                            placeholder: "例如：A 座 1 号客梯",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "elevator-location", "位置描述 *" }
                        Input {
                            id: "elevator-location",
                            value: location(),
                            maxlength: 200,
                            placeholder: "例如：东侧大厅",
                            oninput: move |event: FormEvent| location.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "elevator-size", "轿厢尺寸" }
                        Input {
                            id: "elevator-size",
                            value: size(),
                            maxlength: 100,
                            placeholder: "长 × 宽 × 高",
                            oninput: move |event: FormEvent| size.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "elevator-load", "额定承重（kg）" }
                        Input {
                            id: "elevator-load",
                            value: load_capacity(),
                            inputmode: "decimal",
                            placeholder: "例如：2000",
                            oninput: move |event: FormEvent| load_capacity.set(event.value()),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "生产日期" }
                        DateField {
                            value: production_date(),
                            on_change: move |value: String| production_date.set(value),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "elevator-remark", "备注" }
                        Textarea {
                            id: "elevator-remark",
                            maxlength: 200,
                            rows: 2,
                            value: remark(),
                            placeholder: "维保单位、联系电话等",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                }
                div { class: "field is-wide",
                    ImageEditor {
                        images,
                        title: "设备图片".to_string(),
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
                            let factory_id = match factory_id().parse::<u64>() {
                                Ok(value) if value != 0 => value,
                                _ => {
                                    error.set(Some("请选择所在厂房".into()));
                                    return;
                                }
                            };
                            let name_value = name().trim().to_string();
                            if name_value.is_empty() {
                                error.set(Some("请输入设备名称".into()));
                                return;
                            }
                            let location_value = location().trim().to_string();
                            if location_value.is_empty() {
                                error.set(Some("请输入位置描述".into()));
                                return;
                            }
                            let load_capacity_centi_kg = match parse_load_kg(&load_capacity()) {
                                Ok(value) => value,
                                Err(message) => {
                                    error.set(Some(message));
                                    return;
                                }
                            };
                            let input = ElevatorAssetInput {
                                elevator_name: name_value,
                                location: location_value,
                                size: (!size().trim().is_empty())
                                    .then(|| size().trim().to_string()),
                                load_capacity_centi_kg,
                                production_date: (!production_date().trim().is_empty())
                                    .then(|| production_date().trim().to_string()),
                                remark: (!remark().trim().is_empty())
                                    .then(|| remark().trim().to_string()),
                                factory_id,
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
                                        Ok(image) => uploads.push(UploadedMaintenanceImageInput {
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
                                let saved = save_elevator_asset_record(
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

/// 电梯巡检表单。后台弹窗与手机巡检页共用。
#[component]
pub(super) fn ElevatorInspectionForm(
    asset: ElevatorAsset,
    on_saved: EventHandler<()>,
    on_cancel: Option<EventHandler<()>>,
) -> Element {
    let mut status = use_signal_sync(|| "正常".to_string());
    let mut abnormal_note = use_signal_sync(String::new);
    let mut remark = use_signal_sync(String::new);
    let mut check_time = use_signal_sync(|| datetime_input(now_timestamp()));
    let images = use_signal_sync(Vec::<BusinessImage>::new);
    let mut loading = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let asset_id = asset.asset_id;

    let check_date = use_memo(move || {
        check_time()
            .split('T')
            .next()
            .unwrap_or_default()
            .to_string()
    });
    let check_clock = use_memo(move || {
        check_time()
            .split('T')
            .nth(1)
            .unwrap_or_default()
            .to_string()
    });

    rsx! {
        div { class: "stack",
            div { class: "field",
                span { class: "field-label", "运行状态 *" }
                div { class: "row",
                    for option in ["正常", "异常"] {
                        label {
                            key: "elevator-inspection-status-{option}",
                            class: if status() == option { "tile is-selected" } else { "tile" },
                            input {
                                r#type: "radio",
                                name: "elevator-inspection-status",
                                checked: status() == option,
                                onchange: {
                                    let option = option.to_string();
                                    move |_| status.set(option.clone())
                                },
                            }
                            span { class: "tile-label", "{option}" }
                        }
                    }
                }
            }
            if status() == "异常" {
                div { class: "field is-wide",
                    Label { html_for: "elevator-inspection-abnormal", "异常描述 *" }
                    Textarea {
                        id: "elevator-inspection-abnormal",
                        maxlength: 200,
                        rows: 3,
                        value: abnormal_note(),
                        placeholder: "描述异常现象，例如：异响、平层不准",
                        oninput: move |event: FormEvent| abnormal_note.set(event.value()),
                    }
                }
            }
            div { class: "field",
                span { class: "field-label", "巡检时间 *" }
                div { class: "row",
                    DateField {
                        value: check_date(),
                        on_change: move |value: String| {
                            check_time.set(format!("{value}T{}", check_clock()))
                        },
                    }
                    TimeField {
                        value: check_clock(),
                        on_change: move |value: String| {
                            check_time.set(format!("{}T{value}", check_date()))
                        },
                    }
                }
            }
            div { class: "field is-wide",
                Label { html_for: "elevator-inspection-remark", "备注" }
                Textarea {
                    id: "elevator-inspection-remark",
                    maxlength: 200,
                    rows: 2,
                    value: remark(),
                    placeholder: "随行人员、处理建议等",
                    oninput: move |event: FormEvent| remark.set(event.value()),
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
                if let Some(cancel) = on_cancel {
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        disabled: loading(),
                        onclick: move |_| cancel.call(()),
                        "取消"
                    }
                }
                Button {
                    r#type: "button",
                    disabled: loading(),
                    onclick: move |_| {
                        if loading() {
                            return;
                        }
                        let status_value = status();
                        let note = abnormal_note().trim().to_string();
                        if status_value == "异常" && note.is_empty() {
                            error.set(Some("状态为异常时必须填写异常描述".into()));
                            return;
                        }
                        let check_time_value = match parse_datetime(&check_time()) {
                            Ok(value) => value,
                            Err(message) => {
                                error.set(Some(message));
                                return;
                            }
                        };
                        let pending_files = images()
                            .into_iter()
                            .filter_map(|image| match image {
                                BusinessImage::Pending { file, .. } => Some(file),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        let input = ElevatorInspectionInput {
                            asset_id,
                            status: status_value,
                            abnormal_note: (!note.is_empty()).then_some(note),
                            check_time: Some(check_time_value),
                            remark: (!remark().trim().is_empty())
                                .then(|| remark().trim().to_string()),
                        };
                        error.set(None);
                        loading.set(true);
                        let images = images;
                        spawn(async move {
                            let mut uploads = Vec::with_capacity(pending_files.len());
                            for file in pending_files {
                                match upload_business_image(file).await {
                                    Ok(image) => uploads.push(UploadedMaintenanceImageInput {
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
                            match create_elevator_inspection_record(input, uploads).await {
                                Ok(()) => {
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
                    if loading() { "提交中…" } else { "提交巡检" }
                }
            }
        }
    }
}

#[component]
fn ElevatorHistoryDialog(
    asset: ElevatorAsset,
    inspections: Vec<ElevatorInspection>,
    previews: Vec<ElevatorInspectionImagePreview>,
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
            DialogTitle { "巡检历史 · {asset.elevator_name}" }
            DialogDescription { "共 {inspections.len()} 次巡检，按时间倒序。记录只增不删。" }
            if inspections.is_empty() {
                p { class: "empty", "这台设备还没有巡检记录。" }
            } else {
                div { class: "stack",
                    for row in inspections {
                        div { key: "{row.inspection_id}", class: "history-item",
                            div { class: "stack-tight",
                                div { class: "row",
                                    Badge { variant: status_variant(&row.status), "{row.status}" }
                                    span { class: "is-mono", "{format_datetime(row.check_time)}" }
                                    span { class: "hint", "巡检人 {row.inspector_name}" }
                                }
                                if let Some(note) = &row.abnormal_note {
                                    p { class: "form-error", "异常：{note}" }
                                }
                                if let Some(note) = &row.remark {
                                    small { class: "hint", "{note}" }
                                }
                                {
                                    let photos = previews
                                        .iter()
                                        .filter(|preview| preview.inspection_id == row.inspection_id)
                                        .cloned()
                                        .collect::<Vec<_>>();
                                    rsx! {
                                        if !photos.is_empty() {
                                            div { class: "row",
                                                for photo in photos {
                                                    img {
                                                        key: "{photo.img_id}",
                                                        class: "history-photo",
                                                        src: "{photo.img_url}",
                                                        alt: "巡检照片",
                                                        loading: "lazy",
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ElevatorQrDialog(asset: ElevatorAsset, on_close: EventHandler<()>) -> Element {
    let url = inspect_url("elevator", asset.asset_id);
    let svg = qr_svg(&url);
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "巡检码 · {asset.elevator_name}" }
            DialogDescription { "打印后贴在设备上；手机扫码即可打开这台电梯的巡检页。码里只有设备编号，不含任何密钥——能否填报由登录账号的巡检权限决定。" }
            div { class: "stack qr-stack",
                if let Some(svg) = svg {
                    div { class: "qr-code", dangerous_inner_html: "{svg}" }
                } else {
                    p { class: "form-error", "二维码生成失败" }
                }
                div { class: "stack-tight",
                    strong { "{asset.elevator_name}" }
                    small { class: "hint", "{asset.location}" }
                    small { class: "hint is-mono", "{url}" }
                }
                div { class: "form-actions",
                    Button {
                        variant: ButtonVariant::Outline,
                        r#type: "button",
                        onclick: move |_| on_close.call(()),
                        "关闭"
                    }
                    Button { r#type: "button", onclick: move |_| print_qr_sheet(), "打印" }
                }
            }
            div { class: "qr-print-sheet",
                QrPrintLabel {
                    title: asset.elevator_name.clone(),
                    code: format!("LIFT-{:06}", asset.asset_id),
                    location: asset.location.clone(),
                    url: url.clone(),
                }
            }
        }
    }
}
