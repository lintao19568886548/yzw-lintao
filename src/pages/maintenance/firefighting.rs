//! 消防设施资产台账页：一行一个设施。
//!
//! 与变压器、电梯台账同构，自身差异两条：设施必须挂在**厂房楼层或宿舍楼层**
//! 之一上（表单为两段级联，园区由服务端沿楼层推导）；灭火器带**有效期至**，
//! 已到期/临期设施在页顶横幅与行内徽标上提醒——巡检人员打开即见。
//! 设计见 `docs/消防设施台账与扫码巡检.md`。

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
        create_firefighting_inspection_record, delete_business_images_from_r2,
        delete_firefighting_asset_record, save_firefighting_asset_record, upload_business_image,
        StoredR2Image,
    },
    spacetime_bindings::{
        dormitory_floor_type::DormitoryFloor, dormitory_type::Dormitory,
        factory_floor_type::FactoryFloor, factory_type::Factory,
        firefighting_asset_image_preview_type::FirefightingAssetImagePreview,
        firefighting_asset_input_type::FirefightingAssetInput,
        firefighting_asset_type::FirefightingAsset,
        firefighting_inspection_image_preview_type::FirefightingInspectionImagePreview,
        firefighting_inspection_input_type::FirefightingInspectionInput,
        firefighting_inspection_type::FirefightingInspection, park_type::Park,
        uploaded_maintenance_image_input_type::UploadedMaintenanceImageInput,
    },
    state::WorkspaceState,
};

const PAGE_SIZE: usize = 20;

/// 与服务端 `MAX_MAINTENANCE_IMAGES` 保持一致。
const MAX_IMAGES: usize = 8;

/// 临期阈值：有效期 30 天内开始提醒。
const EXPIRY_SOON_DAYS: i64 = 30;

#[derive(Clone, PartialEq)]
struct AssetRow {
    asset: FirefightingAsset,
    latest: Option<FirefightingInspection>,
    /// 有效期距今天数；`None` = 未填或非法。
    days_left: Option<i64>,
}

impl AssetRow {
    fn status_label(&self) -> &str {
        self.latest
            .as_ref()
            .map(|row| row.status.as_str())
            .unwrap_or("未巡检")
    }

    fn expired(&self) -> bool {
        self.days_left.is_some_and(|days| days < 0)
    }

    fn expiring_soon(&self) -> bool {
        self.days_left
            .is_some_and(|days| (0..=EXPIRY_SOON_DAYS).contains(&days))
    }
}

/// 设施位置展示：园区 · 楼宇 · 楼层。
pub(super) fn location_label(
    asset: &FirefightingAsset,
    parks: &[Park],
    factories: &[Factory],
    factory_floors: &[FactoryFloor],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
) -> String {
    let park = park_name(parks, asset.park_id);
    if asset.factory_floor_id != 0 {
        let floor = factory_floors
            .iter()
            .find(|row| row.floor_id == asset.factory_floor_id);
        let factory = floor
            .map(|row| factory_name(factories, Some(row.factory_id)))
            .unwrap_or_else(|| "未知厂房".into());
        let floor_name = floor
            .map(|row| row.floor_name.clone())
            .unwrap_or_else(|| format!("楼层 #{}", asset.factory_floor_id));
        return format!("{park} · {factory} · {floor_name}");
    }
    let floor = dormitory_floors
        .iter()
        .find(|row| row.dormitory_floor_id == asset.dormitory_floor_id);
    let dormitory = floor
        .and_then(|row| {
            dormitories
                .iter()
                .find(|dorm| dorm.dormitory_id == row.dormitory_id)
        })
        .map(|dorm| dorm.dormitory_name.clone())
        .unwrap_or_else(|| "未知宿舍".into());
    let floor_no = floor
        .map(|row| format!("{} 层", row.floor_no))
        .unwrap_or_else(|| format!("楼层 #{}", asset.dormitory_floor_id));
    format!("{park} · {dormitory} · {floor_no}")
}

#[component]
/// `factory` 来自 URL 查询参数：从园区档案页的设施概览卡跳来时预填搜索框，
/// 落地即是该厂房的设备（园区管理.md §2.3.1）。直接打开本页时为空串。
pub fn MaintenanceFirefightingPage(factory: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut query = use_signal(|| factory.clone());
    let mut type_filter = use_signal(|| "全部".to_string());
    let mut status_filter = use_signal(|| "全部".to_string());
    let mut page = use_signal(|| 1usize);
    let mut feedback = use_signal_sync(|| None::<String>);
    // Some(None) = 新增，Some(Some(asset)) = 编辑。
    let mut asset_form = use_signal(|| None::<Option<FirefightingAsset>>);
    let mut inspect_target = use_signal(|| None::<FirefightingAsset>);
    let mut history_target = use_signal(|| None::<FirefightingAsset>);
    let mut qr_target = use_signal(|| None::<FirefightingAsset>);
    let mut confirm_delete = use_signal(|| None::<FirefightingAsset>);
    let mut batch_print = use_signal(|| false);
    let deleting = use_signal_sync(|| false);

    let parks = state.parks.read().clone();
    let factories = state.factories.read().clone();
    let factory_floors = state.factory_floors.read().clone();
    let dormitories = state.dormitories.read().clone();
    let dormitory_floors = state.dormitory_floors.read().clone();
    let inspections = state.firefighting_inspections.read().clone();
    let can_inspect = can_submit_inspection(&state);
    let today = today_date();

    let all_rows = state
        .firefighting_assets
        .read()
        .iter()
        .map(|asset| AssetRow {
            latest: latest_firefighting_inspection(&inspections, asset.asset_id),
            days_left: asset
                .expiry_on
                .as_deref()
                .and_then(|value| expiry_days_left(value, &today)),
            asset: asset.clone(),
        })
        .collect::<Vec<_>>();
    let total_assets = all_rows.len();
    let abnormal = all_rows
        .iter()
        .filter(|row| row.status_label() == "异常")
        .count();
    let expired = all_rows.iter().filter(|row| row.expired()).count();

    let query_value = query().trim().to_lowercase();
    let type_value = type_filter();
    let status_value = status_filter();
    let rows = all_rows
        .into_iter()
        .filter(|row| {
            let status_match = match status_value.as_str() {
                "全部" => true,
                "已到期" => row.expired(),
                "临期" => row.expiring_soon(),
                value => row.status_label() == value,
            };
            (type_value == "全部" || row.asset.facility_type == type_value)
                && status_match
                && (query_value.is_empty()
                    || format!(
                        "{} {} {} {}",
                        row.asset.facility_name,
                        row.asset.location,
                        row.asset.specifications.clone().unwrap_or_default(),
                        location_label(
                            &row.asset,
                            &parks,
                            &factories,
                            &factory_floors,
                            &dormitories,
                            &dormitory_floors,
                        ),
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
    let type_value_signal: ReadSignal<Option<String>> =
        use_memo(move || Some(type_filter())).into();
    let status_value_signal: ReadSignal<Option<String>> =
        use_memo(move || Some(status_filter())).into();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "消防管理" }
                    p { class: "page-subtitle", "一行一个设施，挂在厂房或宿舍的楼层上。灭火器按有效期自动提醒。" }
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
                        "新增设施"
                    }
                }
            }

            if let Some(message) = feedback() {
                p { class: "notice", role: "status", "{message}" }
            }

            if expired > 0 {
                p { class: "notice", role: "alert",
                    strong { class: "is-bad", "{expired} 个设施已过有效期" }
                    "，请及时更换后更新台账。"
                }
            }

            section { class: "grid-3", aria_label: "消防台账总览",
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "在册设施" }
                    strong { class: "stat-value is-mono", "{total_assets}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "最近巡检异常" }
                    strong { class: "stat-value is-mono is-bad", "{abnormal}" }
                } } } }
                div { Card { CardContent { div { class: "stat",
                    span { class: "stat-label", "已到期" }
                    strong { class: "stat-value is-mono is-bad", "{expired}" }
                } } } }
            }

            section { class: "section",
                Card { CardContent { div { class: "filters",
                    div { class: "field",
                        Label { html_for: "firefighting-query", "搜索" }
                        Input {
                            id: "firefighting-query",
                            value: query,
                            placeholder: "名称、位置、型号、楼宇或园区",
                            oninput: move |event: FormEvent| {
                                query.set(event.value());
                                page.set(1);
                            },
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-type", "设施类型" }
                        Select {
                            id: "firefighting-type",
                            value: Some(type_value_signal),
                            on_value_change: move |value: Option<String>| {
                                type_filter.set(value.unwrap_or_else(|| "全部".into()));
                                page.set(1);
                            },
                            SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部类型".to_string(), "全部类型" }
                            for (index , value) in FIREFIGHTING_TYPES.into_iter().enumerate() {
                                SelectOption::<String> {
                                    key: "firefighting-type-{value}",
                                    value: value.to_string(),
                                    index: index + 1,
                                    text_value: value.to_string(),
                                    "{value}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-status", "状态" }
                        Select {
                            id: "firefighting-status",
                            value: Some(status_value_signal),
                            on_value_change: move |value: Option<String>| {
                                status_filter.set(value.unwrap_or_else(|| "全部".into()));
                                page.set(1);
                            },
                            SelectOption::<String> { value: "全部".to_string(), index: 0usize, text_value: "全部".to_string(), "全部" }
                            for (index , value) in ["正常", "异常", "未巡检", "已到期", "临期"].into_iter().enumerate() {
                                SelectOption::<String> {
                                    key: "firefighting-status-{value}",
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
                    h2 { "消防管理台账" }
                    Badge { variant: BadgeVariant::Secondary, "{total} 个" }
                }
                if visible.is_empty() {
                    p { class: "empty", "暂无符合条件的设施。先在这里逐个录入消防设施，再打印巡检码贴到设施旁。" }
                } else {
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "设施" }
                                    th { "位置" }
                                    th { "型号 / 有效期" }
                                    th { "最近巡检" }
                                    th { "操作" }
                                }
                            }
                            tbody {
                                for row in visible {
                                    tr { key: "{row.asset.asset_id}",
                                        td {
                                            div { class: "stack-tight",
                                                div { class: "row",
                                                    Badge { variant: BadgeVariant::Outline, "{row.asset.facility_type}" }
                                                    strong { "{row.asset.facility_name}" }
                                                }
                                                small { class: "hint is-mono", "FIRE-{row.asset.asset_id:06}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span {
                                                    {location_label(
                                                        &row.asset,
                                                        &parks,
                                                        &factories,
                                                        &factory_floors,
                                                        &dormitories,
                                                        &dormitory_floors,
                                                    )}
                                                }
                                                small { class: "hint", "{row.asset.location}" }
                                            }
                                        }
                                        td {
                                            div { class: "stack-tight",
                                                span { {row.asset.specifications.clone().unwrap_or_else(|| "型号未记录".into())} }
                                                if let Some(expiry) = &row.asset.expiry_on {
                                                    if row.expired() {
                                                        Badge { variant: BadgeVariant::Destructive, "已到期 {expiry}" }
                                                    } else if let Some(days) = row.days_left.filter(|days| *days <= EXPIRY_SOON_DAYS) {
                                                        Badge { variant: BadgeVariant::Outline, "{days} 天后到期" }
                                                    } else {
                                                        small { class: "hint", "有效期至 {expiry}" }
                                                    }
                                                } else {
                                                    small { class: "hint", "无有效期" }
                                                }
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
            FirefightingAssetDialog {
                asset: editing.clone(),
                previews: editing
                    .as_ref()
                    .map(|asset| {
                        state
                            .firefighting_asset_image_previews
                            .read()
                            .iter()
                            .filter(|preview| preview.asset_id == asset.asset_id)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
                parks: parks.clone(),
                factories: factories.clone(),
                factory_floors: factory_floors.clone(),
                dormitories: dormitories.clone(),
                dormitory_floors: dormitory_floors.clone(),
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
                DialogTitle { "登记巡检 · {asset.facility_name}" }
                DialogDescription { "巡检记录只增不删，填错请补一条更正记录。" }
                FirefightingInspectionForm {
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
            FirefightingHistoryDialog {
                asset: asset.clone(),
                inspections: inspections
                    .iter()
                    .filter(|row| row.asset_id == asset.asset_id)
                    .cloned()
                    .collect::<Vec<_>>(),
                previews: state
                    .firefighting_inspection_image_previews
                    .read()
                    .clone(),
                on_close: move |_| history_target.set(None),
            }
        }

        if let Some(asset) = qr_target() {
            FirefightingQrDialog { asset, on_close: move |_| qr_target.set(None) }
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
                DialogDescription { "共 {print_rows.len()} 个设施（按当前筛选）。打印时只输出标签区域，裁开后贴到设施旁。" }
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
                                title: format!("{} · {}", row.asset.facility_type, row.asset.facility_name),
                                code: format!("FIRE-{:06}", row.asset.asset_id),
                                location: row.asset.location.clone(),
                                url: inspect_url("firefighting", row.asset.asset_id),
                            }
                        }
                    }
                }
            }
        }

        if let Some(asset) = confirm_delete() {
            ConfirmDialog {
                title: "确认注销这个消防设施？",
                description: "这是逻辑删除：设施与巡检历史从台账隐藏，数据不会被真正删除。",
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
                        let result = delete_firefighting_asset_record(asset_id).await;
                        deleting.set(false);
                        confirm_delete.set(None);
                        feedback.set(Some(match result {
                            Ok(()) => "设施已注销".into(),
                            Err(error) => error,
                        }));
                    });
                },
            }
        }
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
fn FirefightingAssetDialog(
    asset: Option<FirefightingAsset>,
    previews: Vec<FirefightingAssetImagePreview>,
    parks: Vec<Park>,
    factories: Vec<Factory>,
    factory_floors: Vec<FactoryFloor>,
    dormitories: Vec<Dormitory>,
    dormitory_floors: Vec<DormitoryFloor>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let asset_id = asset.as_ref().map(|row| row.asset_id);
    // 编辑时沿楼层反推楼宇与园区，回填级联选择器。
    let initial_kind = asset
        .as_ref()
        .map(|row| {
            if row.dormitory_floor_id != 0 {
                "dormitory".to_string()
            } else {
                "factory".to_string()
            }
        })
        .unwrap_or_else(|| "factory".into());
    let initial_building = asset
        .as_ref()
        .map(|row| {
            if row.factory_floor_id != 0 {
                factory_floors
                    .iter()
                    .find(|floor| floor.floor_id == row.factory_floor_id)
                    .map(|floor| floor.factory_id.to_string())
                    .unwrap_or_default()
            } else {
                dormitory_floors
                    .iter()
                    .find(|floor| floor.dormitory_floor_id == row.dormitory_floor_id)
                    .map(|floor| floor.dormitory_id.to_string())
                    .unwrap_or_default()
            }
        })
        .unwrap_or_default();
    let initial_floor = asset
        .as_ref()
        .map(|row| {
            if row.factory_floor_id != 0 {
                row.factory_floor_id.to_string()
            } else {
                row.dormitory_floor_id.to_string()
            }
        })
        .unwrap_or_default();
    let mut park_id = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.park_id.to_string())
            .unwrap_or_default()
    });
    let mut building_kind = use_signal_sync(|| initial_kind);
    let mut building_id = use_signal_sync(|| initial_building);
    let mut floor_id = use_signal_sync(|| initial_floor);
    let mut facility_type = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.facility_type.clone())
            .unwrap_or_else(|| "灭火器".into())
    });
    let mut name = use_signal_sync(|| {
        asset
            .as_ref()
            .map(|row| row.facility_name.clone())
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
            .and_then(|row| row.specifications.clone())
            .unwrap_or_default()
    });
    let mut expiry_on = use_signal_sync(|| {
        asset
            .as_ref()
            .and_then(|row| row.expiry_on.clone())
            .unwrap_or_default()
    });
    let mut remark = use_signal_sync(|| {
        asset
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

    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();
    let building_value: ReadSignal<Option<String>> = use_memo(move || Some(building_id())).into();
    let floor_value: ReadSignal<Option<String>> = use_memo(move || Some(floor_id())).into();
    let type_value: ReadSignal<Option<String>> = use_memo(move || Some(facility_type())).into();

    let selected_park = park_id().parse::<u64>().ok();
    let is_dormitory = building_kind() == "dormitory";
    let selected_building = building_id().parse::<u64>().ok();
    let park_factories = factories
        .iter()
        .filter(|row| selected_park == Some(row.park_id))
        .cloned()
        .collect::<Vec<_>>();
    let park_dormitories = dormitories
        .iter()
        .filter(|row| selected_park == Some(row.park_id) && !row.is_deleted)
        .cloned()
        .collect::<Vec<_>>();
    let building_factory_floors = factory_floors
        .iter()
        .filter(|row| selected_building == Some(row.factory_id) && !row.is_deleted)
        .cloned()
        .collect::<Vec<_>>();
    let building_dormitory_floors = dormitory_floors
        .iter()
        .filter(|row| selected_building == Some(row.dormitory_id) && !row.is_deleted)
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
            DialogTitle { if asset_id.is_some() { "编辑消防设施" } else { "新增消防设施" } }
            DialogDescription { "设施必须挂在厂房或宿舍的楼层上，园区随楼层自动确定；灭火器必须填写有效期。" }
            div { class: "stack",
                div { class: "form-grid",
                    div { class: "field",
                        span { class: "field-label", "所在楼宇类型 *" }
                        div { class: "row",
                            for (value , label) in [("factory", "厂房"), ("dormitory", "宿舍楼")] {
                                label {
                                    key: "firefighting-kind-{value}",
                                    class: if building_kind() == value { "tile is-selected" } else { "tile" },
                                    input {
                                        r#type: "radio",
                                        name: "firefighting-kind",
                                        checked: building_kind() == value,
                                        onchange: {
                                            let value = value.to_string();
                                            move |_| {
                                                building_kind.set(value.clone());
                                                building_id.set(String::new());
                                                floor_id.set(String::new());
                                            }
                                        },
                                    }
                                    span { class: "tile-label", "{label}" }
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-park", "所属园区（筛选用）" }
                        Select {
                            id: "firefighting-park",
                            value: Some(park_value),
                            on_value_change: move |value: Option<String>| {
                                park_id.set(value.unwrap_or_default());
                                building_id.set(String::new());
                                floor_id.set(String::new());
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择园区".to_string(), "请选择园区" }
                            for (index , park) in parks.iter().enumerate() {
                                SelectOption::<String> {
                                    key: "firefighting-park-{park.park_id}",
                                    value: park.park_id.to_string(),
                                    index: index + 1,
                                    text_value: park.park_name.to_string(),
                                    "{park.park_name}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-building", if is_dormitory { "所在宿舍楼 *" } else { "所在厂房 *" } }
                        Select {
                            id: "firefighting-building",
                            value: Some(building_value),
                            on_value_change: move |value: Option<String>| {
                                building_id.set(value.unwrap_or_default());
                                floor_id.set(String::new());
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择楼宇".to_string(), "请选择楼宇" }
                            if is_dormitory {
                                for (index , dormitory) in park_dormitories.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "firefighting-dormitory-{dormitory.dormitory_id}",
                                        value: dormitory.dormitory_id.to_string(),
                                        index: index + 1,
                                        text_value: dormitory.dormitory_name.to_string(),
                                        "{dormitory.dormitory_name}"
                                    }
                                }
                            } else {
                                for (index , factory) in park_factories.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "firefighting-factory-{factory.factory_id}",
                                        value: factory.factory_id.to_string(),
                                        index: index + 1,
                                        text_value: factory.factory_name.to_string(),
                                        "{factory.factory_name}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-floor", "所在楼层 *" }
                        Select {
                            id: "firefighting-floor",
                            value: Some(floor_value),
                            on_value_change: move |value: Option<String>| {
                                floor_id.set(value.unwrap_or_default())
                            },
                            SelectOption::<String> { value: String::new(), index: 0usize, text_value: "请选择楼层".to_string(), "请选择楼层" }
                            if is_dormitory {
                                for (index , floor) in building_dormitory_floors.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "firefighting-dfloor-{floor.dormitory_floor_id}",
                                        value: floor.dormitory_floor_id.to_string(),
                                        index: index + 1,
                                        text_value: format!("{} 层", floor.floor_no),
                                        "{floor.floor_no} 层"
                                    }
                                }
                            } else {
                                for (index , floor) in building_factory_floors.iter().enumerate() {
                                    SelectOption::<String> {
                                        key: "firefighting-ffloor-{floor.floor_id}",
                                        value: floor.floor_id.to_string(),
                                        index: index + 1,
                                        text_value: floor.floor_name.to_string(),
                                        "{floor.floor_name}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-type-select", "设施类型 *" }
                        Select {
                            id: "firefighting-type-select",
                            value: Some(type_value),
                            on_value_change: move |value: Option<String>| {
                                facility_type.set(value.unwrap_or_else(|| "灭火器".into()))
                            },
                            for (index , value) in FIREFIGHTING_TYPES.into_iter().enumerate() {
                                SelectOption::<String> {
                                    key: "firefighting-type-option-{value}",
                                    value: value.to_string(),
                                    index,
                                    text_value: value.to_string(),
                                    "{value}"
                                }
                            }
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-name", "编号或名称 *" }
                        Input {
                            id: "firefighting-name",
                            value: name(),
                            maxlength: 100,
                            placeholder: "例如：3F 东侧灭火器 2 号",
                            oninput: move |event: FormEvent| name.set(event.value()),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "firefighting-location", "楼层内位置 *" }
                        Input {
                            id: "firefighting-location",
                            value: location(),
                            maxlength: 200,
                            placeholder: "例如：东侧楼梯口",
                            oninput: move |event: FormEvent| location.set(event.value()),
                        }
                    }
                    div { class: "field",
                        Label { html_for: "firefighting-spec", "型号规格" }
                        Input {
                            id: "firefighting-spec",
                            value: specifications(),
                            maxlength: 100,
                            placeholder: "例如：MFZ/ABC4 干粉",
                            oninput: move |event: FormEvent| specifications.set(event.value()),
                        }
                    }
                    div { class: "field",
                        span { class: "field-label",
                            if facility_type() == "灭火器" { "有效期至 *" } else { "有效期至" }
                        }
                        DateField {
                            value: expiry_on(),
                            on_change: move |value: String| expiry_on.set(value),
                        }
                    }
                    div { class: "field is-wide",
                        Label { html_for: "firefighting-remark", "备注" }
                        Textarea {
                            id: "firefighting-remark",
                            maxlength: 200,
                            rows: 2,
                            value: remark(),
                            placeholder: "维保信息、采购批次等",
                            oninput: move |event: FormEvent| remark.set(event.value()),
                        }
                    }
                }
                div { class: "field is-wide",
                    ImageEditor {
                        images,
                        title: "设施图片".to_string(),
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
                            let floor = match floor_id().parse::<u64>() {
                                Ok(value) if value != 0 => value,
                                _ => {
                                    error.set(Some("请选择所在楼层".into()));
                                    return;
                                }
                            };
                            let (factory_floor_id, dormitory_floor_id) =
                                if building_kind() == "dormitory" {
                                    (0, floor)
                                } else {
                                    (floor, 0)
                                };
                            let name_value = name().trim().to_string();
                            if name_value.is_empty() {
                                error.set(Some("请输入设施编号或名称".into()));
                                return;
                            }
                            let location_value = location().trim().to_string();
                            if location_value.is_empty() {
                                error.set(Some("请输入楼层内位置".into()));
                                return;
                            }
                            let type_value = facility_type();
                            let expiry_value = expiry_on().trim().to_string();
                            if type_value == "灭火器" && expiry_value.is_empty() {
                                error.set(Some("灭火器必须填写有效期至".into()));
                                return;
                            }
                            let input = FirefightingAssetInput {
                                facility_type: type_value,
                                facility_name: name_value,
                                location: location_value,
                                specifications: (!specifications().trim().is_empty())
                                    .then(|| specifications().trim().to_string()),
                                expiry_on: (!expiry_value.is_empty()).then_some(expiry_value),
                                remark: (!remark().trim().is_empty())
                                    .then(|| remark().trim().to_string()),
                                factory_floor_id,
                                dormitory_floor_id,
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
                                let saved = save_firefighting_asset_record(
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

/// 消防巡检表单。后台弹窗与手机巡检页共用。
#[component]
pub(super) fn FirefightingInspectionForm(
    asset: FirefightingAsset,
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
                            key: "firefighting-inspection-status-{option}",
                            class: if status() == option { "tile is-selected" } else { "tile" },
                            input {
                                r#type: "radio",
                                name: "firefighting-inspection-status",
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
                    Label { html_for: "firefighting-inspection-abnormal", "异常描述 *" }
                    Textarea {
                        id: "firefighting-inspection-abnormal",
                        maxlength: 200,
                        rows: 3,
                        value: abnormal_note(),
                        placeholder: "描述异常现象，例如：压力表指针在红区、封条缺失",
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
                Label { html_for: "firefighting-inspection-remark", "备注" }
                Textarea {
                    id: "firefighting-inspection-remark",
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
                        let input = FirefightingInspectionInput {
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
                            match create_firefighting_inspection_record(input, uploads).await {
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
fn FirefightingHistoryDialog(
    asset: FirefightingAsset,
    inspections: Vec<FirefightingInspection>,
    previews: Vec<FirefightingInspectionImagePreview>,
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
            DialogTitle { "巡检历史 · {asset.facility_name}" }
            DialogDescription { "共 {inspections.len()} 次巡检，按时间倒序。记录只增不删。" }
            if inspections.is_empty() {
                p { class: "empty", "这个设施还没有巡检记录。" }
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
fn FirefightingQrDialog(asset: FirefightingAsset, on_close: EventHandler<()>) -> Element {
    let url = inspect_url("firefighting", asset.asset_id);
    let svg = qr_svg(&url);
    rsx! {
        Dialog {
            open: Some(true),
            on_open_change: move |open: bool| {
                if !open {
                    on_close.call(());
                }
            },
            DialogTitle { "巡检码 · {asset.facility_name}" }
            DialogDescription { "打印后贴在设施旁；手机扫码即可打开这个设施的巡检页。码里只有设施编号，不含任何密钥——能否填报由登录账号的巡检权限决定。" }
            div { class: "stack qr-stack",
                if let Some(svg) = svg {
                    div { class: "qr-code", dangerous_inner_html: "{svg}" }
                } else {
                    p { class: "form-error", "二维码生成失败" }
                }
                div { class: "stack-tight",
                    strong { "{asset.facility_name}" }
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
                    title: format!("{} · {}", asset.facility_type, asset.facility_name),
                    code: format!("FIRE-{:06}", asset.asset_id),
                    location: asset.location.clone(),
                    url: url.clone(),
                }
            }
        }
    }
}
