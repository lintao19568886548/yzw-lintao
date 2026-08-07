//! 变压器资产台账页：一行一台真实设备。
//!
//! 与其余维护页面的「记录流水」不同，这里资产与巡检分离：设备本体在
//! `transformer_asset`，巡检事件在 `transformer_inspection` 只增不删地累积。
//! 页面提供资产增改注销、巡检历史时间线、登记巡检与巡检二维码。
//! 设计见 `docs/变压器台账与扫码巡检.md`。

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
        total_pages, BusinessImage, ConfirmDialog, DateField, ImageEditor, Pager, TimeField,
    },
    permissions::is_system_super,
    services::{
        create_transformer_inspection_record, delete_business_images_from_r2,
        delete_transformer_asset_record, save_transformer_asset_record, upload_business_image,
        StoredR2Image,
    },
    spacetime_bindings::{
        factory_type::Factory, park_type::Park,
        transformer_asset_image_preview_type::TransformerAssetImagePreview,
        transformer_asset_input_type::TransformerAssetInput,
        transformer_asset_type::TransformerAsset,
        transformer_inspection_image_preview_type::TransformerInspectionImagePreview,
        transformer_inspection_input_type::TransformerInspectionInput,
        transformer_inspection_type::TransformerInspection,
        uploaded_maintenance_image_input_type::UploadedMaintenanceImageInput,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

/// 与服务端 `MAX_TRANSFORMER_IMAGES` 保持一致。
const MAX_IMAGES: usize = 8;

/// 巡检页 URL。二维码只是入口不是授权，码里只有设备类型与资产 id，没有任何密钥。
pub(super) fn inspect_url(kind: &str, asset_id: u64) -> String {
    #[cfg(target_arch = "wasm32")]
    if let Some(origin) = web_sys::window().and_then(|w| w.location().origin().ok()) {
        return format!("{origin}/maintenance/inspect/{kind}/{asset_id}");
    }
    format!("https://yz.furong.org/maintenance/inspect/{kind}/{asset_id}")
}

/// 生成巡检码 SVG。底色固定为白，扫码可靠性优先，不随主题变色。
pub(super) fn qr_svg(url: &str) -> Option<String> {
    qrcode::QrCode::new(url.as_bytes()).ok().map(|code| {
        code.render::<qrcode::render::svg::Color>()
            .min_dimensions(220, 220)
            .dark_color(qrcode::render::svg::Color("#111111"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build()
    })
}

/// 进入打印：临时给 body 挂上打印模式类，浏览器只输出 `.qr-print-sheet` 区域
/// （样式见 20-app.css「巡检码打印」段），打印对话框关闭后立即还原。
pub(super) fn print_qr_sheet() {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else { return };
        let body = window.document().and_then(|doc| doc.body());
        if let Some(body) = &body {
            let _ = body.class_list().add_1("qr-print-mode");
        }
        let _ = window.print();
        if let Some(body) = &body {
            let _ = body.class_list().remove_1("qr-print-mode");
        }
    }
}

/// 打印标签：二维码 + 名称 + 位置 + 链接，打印后裁开贴到设备上。
#[component]
pub(super) fn QrPrintLabel(title: String, code: String, location: String, url: String) -> Element {
    let svg = qr_svg(&url);
    rsx! {
        div { class: "qr-label",
            if let Some(svg) = svg {
                div { dangerous_inner_html: "{svg}" }
            }
            div { class: "qr-label-name", "{title}" }
            div { class: "qr-label-meta", "{code} · {location}" }
            div { class: "qr-label-meta", "{url}" }
        }
    }
}

/// 当前账号能否提交巡检：Super 或持有巡检权限码。真正的边界在服务端。
pub(super) fn can_submit_inspection(state: &WorkspaceState) -> bool {
    is_system_super(&state.roles.read())
        || state
            .permission_codes
            .read()
            .iter()
            .any(|code| code == MAINTENANCE_INSPECT_CODE)
}

#[derive(Clone, PartialEq)]
struct AssetRow {
    asset: TransformerAsset,
    latest: Option<TransformerInspection>,
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
pub fn MaintenanceTransformerPage(factory: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut query = use_signal(|| factory.clone());
    let mut status_filter = use_signal(|| "全部".to_string());
    let mut page = use_signal(|| 1usize);
    let mut feedback = use_signal_sync(|| None::<String>);
    // Some(None) = 新增，Some(Some(asset)) = 编辑。
    let mut asset_form = use_signal(|| None::<Option<TransformerAsset>>);
    let mut inspect_target = use_signal(|| None::<TransformerAsset>);
    let mut history_target = use_signal(|| None::<TransformerAsset>);
    let mut qr_target = use_signal(|| None::<TransformerAsset>);
    let mut confirm_delete = use_signal(|| None::<TransformerAsset>);
    let mut batch_print = use_signal(|| false);
    let deleting = use_signal_sync(|| false);

    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let inspections = state.transformer_inspections.read().clone();
    let can_inspect = can_submit_inspection(&state);

    let all_rows = state
        .transformer_assets
        .read()
        .iter()
        .map(|asset| AssetRow {
            latest: latest_inspection(&inspections, asset.asset_id),
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
                        row.asset.transformer_name,
                        row.asset.location,
                        row.asset.specifications,
                        park_name(&parks, row.asset.park_id),
                        factory_label(&factories, row.asset.factory_id),
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
                    h1 { "变压器管理" }
                    p { class: "page-subtitle", "一行一台设备。规格、位置、容量是资产信息；运行状态来自最近一次巡检。" }
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
                        "新增变压器"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            section { class: "grid-3", aria_label: "变压器台账总览",
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
                        Label { html_for: "transformer-query", "搜索" }
                        Input {
                            id: "transformer-query",
                            value: query,
                            placeholder: "名称、位置、规格、园区或厂房",
                            oninput: move |event: FormEvent| {
                                query.set(event.value());
                                page.set(1);
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "transformer-status", "最近巡检状态" }
                        Select {
                            id: "transformer-status",
                            value: Some(status_value_signal),
                            on_value_change: move |value: Option<String>| {
                                status_filter.set(value.unwrap_or_else(|| "全部".into()));
                                page.set(1);
                            },
                            SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                            for (index , value) in ["正常", "异常", "未巡检"].into_iter().enumerate() {
                                SelectOption::<String> {
                                    key: "transformer-status-{value}",
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
                    h2 { "变压器管理台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 台" }
                }
                if visible.is_empty() {
                    p { class: "empty", "暂无符合条件的设备。先在这里录入变压器资产，再打印巡检码贴到设备上。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "设备" }
                                    th { "位置" }
                                    th { "规格 / 容量" }
                                    th { "最近巡检" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in visible {
                                    tr { key: "{row.asset.asset_id}",
                                        td {
                                            div { class: "stack-tight",
                                                strong { "{row.asset.transformer_name}" }
                                                small { class: "hint is-mono", "POWER-{row.asset.asset_id:06}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { "{park_name(&parks, row.asset.park_id)} · {factory_label(&factories, row.asset.factory_id)}" }
                                                small { class: "hint", "{row.asset.location}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { "{row.asset.specifications}" }
                                                small { class: "hint", "{format_capacity_kw(row.asset.capacity_centi_kw)}" }
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
            TransformerAssetDialog {
                asset: editing.clone(),
                previews: editing
                    .as_ref()
                    .map(|asset| {
                        state
                            .transformer_asset_image_previews
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
                DialogTitle { "登记巡检 · {asset.transformer_name}" }
                DialogDescription { "巡检记录只增不删，填错请补一条更正记录。" }
                TransformerInspectionForm {
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
            TransformerHistoryDialog {
                asset: asset.clone(),
                inspections: inspections
                    .iter()
                    .filter(|row| row.asset_id == asset.asset_id)
                    .cloned()
                    .collect::<Vec<_>>(),
                previews: state.transformer_inspection_image_previews.read().clone(),
                on_close: move |_| history_target.set(None),
            }
        }

        if let Some(asset) = qr_target() {
            TransformerQrDialog { asset, on_close: move |_| qr_target.set(None) }
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
                DialogDescription { "共 {print_rows.len()} 台设备（按当前筛选）。打印时只输出标签区域，裁开后贴到设备上。" }
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
                                title: row.asset.transformer_name.clone(),
                                code: format!("POWER-{:06}", row.asset.asset_id),
                                location: row.asset.location.clone(),
                                url: inspect_url("transformer", row.asset.asset_id),
                            }
                        }
                    }
                }
            }
        }

        if let Some(asset) = confirm_delete() {
            ConfirmDialog {
                title: "确认注销这台变压器？",
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
                        let result = delete_transformer_asset_record(asset_id).await;
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

/// 厂房展示名；`0` 表示园区公共区域。
fn factory_label(factories: &[Factory], factory_id: u64) -> String {
    if factory_id == 0 {
        return "园区公共区域".into();
    }
    factory_name(factories, Some(factory_id))
}

#[component]
fn TransformerAssetDialog(
    asset: Option<TransformerAsset>,
    previews: Vec<TransformerAssetImagePreview>,
    parks: Vec<Park>,
    factories: Vec<Factory>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let asset_id = asset.as_ref().map(|row| row.asset_id);
    let mut name = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.transformer_name.clone())
            .unwrap_or_default()
    });
    let mut location = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.location.clone())
            .unwrap_or_default()
    });
    let mut specifications = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.specifications.clone())
            .unwrap_or_default()
    });
    let mut capacity = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| capacity_input(row.capacity_centi_kw))
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
            DialogTitle { if asset_id.is_some() { "编辑变压器" } else { "新增变压器" } }
            DialogDescription { "设备信息与图片一并保存；运行状态不在这里维护，由巡检记录得出。" }
            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        Label { html_for: "asset-park", "所属园区 *" }
                        Select {
                            id: "asset-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                park_id.set(value.unwrap_or_default());
                                // 换园区后原厂房不再属于这个园区，回落到公共区域。
                                factory_id.set("0".into());
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "asset-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "asset-factory", "所在厂房" }
                        Select {
                            id: "asset-factory",
                            value: Some(factory_value),
                            on_value_change: move |value: Option<String>| {
                                factory_id.set(value.unwrap_or_else(|| "0".into()))
                            },
                            SelectOption::<String> { value: "0".to_string(), index: 0usize, text_value: "园区公共区域".to_string(), "园区公共区域" }
                            for (index , factory) in park_factories.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "asset-factory-{factory.factory_id}",
                                    value: factory.factory_id.to_string(),
                                    index: index + 1,
                                    text_value: factory.factory_name.to_string(),
                                    "{factory.factory_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "asset-name", "设备名称 *" }
                        Input {
                            id: "asset-name",
                            value: name(),
                            maxlength: 100,
                            placeholder: "例如：1 号变压器",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "asset-spec", "规格型号 *" }
                        Input {
                            id: "asset-spec",
                            value: specifications(),
                            maxlength: 50,
                            placeholder: "例如：SCB13-1250kVA",
                            oninput: move |event: FormEvent| specifications.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "asset-location", "位置描述 *" }
                        Input {
                            id: "asset-location",
                            value: location(),
                            maxlength: 200,
                            placeholder: "例如：3 号厂房西侧配电房",
                            oninput: move |event: FormEvent| location.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "asset-capacity", "容量（kW）" }
                        Input {
                            id: "asset-capacity",
                            value: capacity(),
                            inputmode: "decimal",
                            placeholder: "例如：11.00",
                            oninput: move |event: FormEvent| capacity.set(event.value()),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label", "投运日期" }
                        DateField {
                            value: commissioned_on(),
                            on_change: move |value: String| commissioned_on.set(value),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "asset-remark", "备注" }
                        Textarea {
                            id: "asset-remark",
                            maxlength: 200,
                            rows: 2,
                            value: remark(),
                            placeholder: "补充产权、维保单位等信息",
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
                            let location_value = location().trim().to_string();
                            if location_value.is_empty() {
                                error.set(Some("请输入位置描述".into()));
                                return;
                            }
                            let spec_value = specifications().trim().to_string();
                            if spec_value.is_empty() {
                                error.set(Some("请输入规格型号".into()));
                                return;
                            }
                            let capacity_centi_kw = match parse_capacity_kw(&capacity()) {
                                Ok(value) => value,
                                Err(message) => {
                                    error.set(Some(message));
                                    return;
                                }
                            };
                            let input = TransformerAssetInput {
                                transformer_name: name_value,
                                location: location_value,
                                specifications: spec_value,
                                capacity_centi_kw,
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
                                let saved = save_transformer_asset_record(
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

/// 巡检表单。后台弹窗与手机巡检页共用，保证两个入口的字段与校验不漂移。
#[component]
pub(super) fn TransformerInspectionForm(
    asset: TransformerAsset,
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
                            key: "inspection-status-{option}",
                            class: if status() == option { "tile is-selected" } else { "tile" },
                            input {
                                r#type: "radio",
                                name: "inspection-status",
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
                    Label { html_for: "inspection-abnormal", "异常描述 *" }
                    Textarea {
                        id: "inspection-abnormal",
                        maxlength: 200,
                        rows: 3,
                        value: abnormal_note(),
                        placeholder: "描述异常现象，例如：油温偏高、外壳渗油",
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
                Label { html_for: "inspection-remark", "备注" }
                Textarea {
                    id: "inspection-remark",
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
                        let input = TransformerInspectionInput {
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
                            match create_transformer_inspection_record(input, uploads).await {
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
fn TransformerHistoryDialog(
    asset: TransformerAsset,
    inspections: Vec<TransformerInspection>,
    previews: Vec<TransformerInspectionImagePreview>,
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
            DialogTitle { "巡检历史 · {asset.transformer_name}" }
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
fn TransformerQrDialog(asset: TransformerAsset, on_close: EventHandler<()>) -> Element {
    let url = inspect_url("transformer", asset.asset_id);
    let svg = qr_svg(&url);
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "巡检码 · {asset.transformer_name}" }
            DialogDescription { "打印后贴在设备上；手机扫码即可打开这台设备的巡检页。码里只有设备编号，不含任何密钥——能否填报由登录账号的巡检权限决定。" }
            div { class: "stack qr-stack",
                if let Some(svg) = svg {
                    div { class: "qr-code", dangerous_inner_html: "{svg}" }
                } else {
                    p { class: "form-error", "二维码生成失败" }
                }
                div { class: "stack-tight",
                    strong { "{asset.transformer_name}" }
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
                    Button {
                        r#type: "button",
                        onclick: move |_| print_qr_sheet(),
                        "打印"
                    }
                }
            }
            div { class: "qr-print-sheet",
                QrPrintLabel {
                    title: asset.transformer_name.clone(),
                    code: format!("POWER-{:06}", asset.asset_id),
                    location: asset.location.clone(),
                    url: url.clone(),
                }
            }
        }
    }
}
