//! 园区详情的图片、厂房、楼层、设施和宿舍展示组件。

use dioxus::prelude::*;

use super::{
    detail_asset_edit::{
        DormitoryFloorInlineForm, DormitoryInlineForm, FactoryInlineForm, FloorInlineForm,
    },
    detail_model::{
        format_centi, format_money, DeviceFacilityView, DormitoryDetailRecord,
        FactoryDetailRecord, FirefightingSummaryView, FloorDetailRecord,
    },
    model::format_area,
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonSize, ButtonVariant},
        card::{Card, CardContent, CardHeader, CardTitle},
        collapsible::{Collapsible, CollapsibleContent, CollapsibleTrigger},
        progress::Progress,
        separator::Separator,
        ConfirmDialog,
    },
    permissions::can_manage_rental,
    services::{
        delete_dormitory_floor_record, delete_dormitory_record, delete_factory_floor_record,
        delete_factory_record,
    },
    spacetime_bindings::{dormitory_type::Dormitory, factory_floor_type::FactoryFloor},
    state::WorkspaceState,
};

/// 当前账号能否维护园区资产，与服务端 `require_rental_manager` 同口径。
fn can_edit_assets() -> bool {
    let state = use_context::<WorkspaceState>();
    can_manage_rental(&(state.roles)(), &(state.menus)())
}

#[component]
pub(super) fn ImageGallery(images: Vec<String>, title: String, compact: bool) -> Element {
    let mut current = use_signal(|| 0usize);
    let mut preview_open = use_signal(|| false);
    let count = images.len();
    let current_index = current().min(count.saturating_sub(1));
    let current_url = images.get(current_index).cloned();
    let gallery_class = if compact {
        "park-detail-gallery is-compact"
    } else {
        "park-detail-gallery"
    };

    rsx! {
        div { class: gallery_class,
            if current_url.is_some() {
                button { class: "park-detail-gallery__image", r#type: "button", aria_label: "预览{title}", onclick: move |_| preview_open.set(true),
                    // 整组图片一次性挂载，切换只改可见性。
                    // 单个 img 换 src 的话，浏览器要先把新图下载解码完才会重绘，
                    // 点了箭头要等一会儿画面才动，像是按钮没反应。
                    for (index , url) in images.iter().enumerate() {
                        img {
                            key: "{url}",
                            src: "{url}",
                            alt: if index == current_index { format!("{title} 图片 {}", index + 1) } else { String::new() },
                            aria_hidden: index != current_index,
                            class: if index == current_index { "is-current" } else { "" },
                            loading: "lazy",
                        }
                    }
                }
                if count > 1 {
                    button { class: "park-detail-gallery__arrow is-prev", r#type: "button", aria_label: "上一张图片", onclick: move |_| current.set(if current_index == 0 { count - 1 } else { current_index - 1 }), "‹" }
                    button { class: "park-detail-gallery__arrow is-next", r#type: "button", aria_label: "下一张图片", onclick: move |_| current.set((current_index + 1) % count), "›" }
                    div { class: "park-detail-gallery__dots", aria_label: "图片列表",
                        for index in 0..count {
                            button { r#type: "button", class: if index == current_index { "is-active" } else { "" }, aria_label: "查看第{index + 1}张图片", onclick: move |_| current.set(index) }
                        }
                    }
                }
            } else {
                div { class: "park-detail-gallery__placeholder", span { "YZ" } small { "PARK ASSET" } }
            }
        }
        if preview_open() {
            div { class: "park-image-preview", role: "dialog", aria_modal: "true", aria_label: "{title}图片预览", onclick: move |_| preview_open.set(false),
                button { r#type: "button", aria_label: "关闭图片预览", onclick: move |_| preview_open.set(false), "×" }
                if let Some(url) = current_url.as_deref() { img { src: "{url}", alt: "{title}", onclick: move |event| event.stop_propagation() } }
            }
        }
    }
}

#[component]
pub(super) fn FactorySection(park_id: u64, factories: Vec<FactoryDetailRecord>) -> Element {
    let can_edit = can_edit_assets();
    let mut adding = use_signal_sync(|| false);
    rsx! {
        div { class: "section",
            Card {
            CardHeader {
                div { class: "section-header",
                    CardTitle { "厂房信息" }
                    div { class: "section-tools",
                    Badge { variant: BadgeVariant::Secondary, "{factories.len()} 个" }
                    if can_edit && !adding() {
                        Button { size: ButtonSize::Sm, r#type: "button", onclick: move |_| adding.set(true), "新增厂房" }
                    }
                    }
                }
            }
            if adding() {
                FactoryInlineForm { park_id, factory: None, on_done: move |_| adding.set(false) }
            }
            CardContent {
                if factories.is_empty() && !adding() {
                    p { class: "empty", "暂无厂房信息" }
                } else {
                    div { class: "stack",
                        for (index, record) in factories.into_iter().enumerate() {
                            FactoryPanel { key: "factory-{record.factory.factory_id}", park_id, record, expanded: index < 3 }
                        }
                    }
                }
            }
            }
        }
    }
}

#[component]
fn FactoryPanel(park_id: u64, record: FactoryDetailRecord, expanded: bool) -> Element {
    let can_edit = can_edit_assets();
    let mut editing = use_signal_sync(|| false);
    let mut confirming = use_signal_sync(|| false);
    let mut deleting = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let factory = &record.factory;
    let factory_id = factory.factory_id;
    let editable_factory = factory.clone();
    let description = factory.description.as_deref().unwrap_or("暂无厂房描述");
    let build_date = factory.build_date.as_deref().unwrap_or("未知");
    let occupancy = if record.total_area > 0 {
        (record.used_area as f64 / record.total_area as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    rsx! {
        Collapsible { default_open: expanded,
            // 操作按钮放在标题行右侧，而不是正文第一行——原来它们悬在字段行
            // 上方，和「楼层可租面积」那一列挤在同一片区域，看着像浮在数据上。
            div { class: "collapse-head",
                CollapsibleTrigger {
                    div { class: "collapse-summary",
                        strong { "{factory.factory_name}" }
                        span { "{record.floors.len()} 个楼层" }
                    }
                }
                if can_edit && !editing() {
                    div { class: "collapse-actions",
                        Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, r#type: "button", onclick: move |_| editing.set(true), "编辑厂房" }
                        Button { variant: ButtonVariant::Outline, class: "is-quiet-danger", size: ButtonSize::Sm, r#type: "button", onclick: move |_| confirming.set(true), "删除厂房" }
                        if confirming() {
                            ConfirmDialog {
                                title: "删除厂房？",
                                description: "该厂房及其全部楼层都会被归档，关联的合同与历史记录仍然保留。",
                                confirm_label: "确认删除",
                                busy: deleting(),
                                error: error(),
                                on_cancel: move |_| confirming.set(false),
                                on_confirm: move |_| {
                                    deleting.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        if let Err(message) = delete_factory_record(factory_id).await {
                                            error.set(Some(message));
                                        }
                                        deleting.set(false);
                                        confirming.set(false);
                                    });
                                },
                            }
                        }
                    }
                }
            }
            // is-boxed 给展开区加左侧边线和浅底：不加包裹的话，展开出来的
            // 楼层、设施和下一个厂房在视觉上是平级的，看不出归属。
            CollapsibleContent { class: "collapse-body is-boxed",
                if let Some(message) = error() { p { class: "form-error", "{message}" } }
                if editing() {
                    FactoryInlineForm { park_id, factory: Some(editable_factory), on_done: move |_| editing.set(false) }
                }

                // 基础字段也放进一个同宽同内边距的外框，和下面的指标磁贴
                // 共用三列网格——否则两排文字的起始位置差一个内边距，看着像
                // 交错的锯齿。
                div { class: "panel-row",
                    div { class: "stack-tight",
                        span { class: "field-label", "建造时间" }
                        strong { class: "is-mono", "{build_date}" }
                    }
                }

                div { class: "grid-3",
                    div { class: "panel is-tight is-plain",
                        span { class: "stat-label", "楼层总面积" }
                        strong { class: "stat-value is-compact is-mono", "{format_area(record.total_area)} ㎡" }
                    }
                    div { class: "panel is-tight is-plain",
                        span { class: "stat-label", "已用面积" }
                        strong { class: "stat-value is-compact is-mono", "{format_area(record.used_area)} ㎡" }
                    }
                    // 可租面积是招商时最先看的数字，用正向状态色标出来
                    div { class: "panel is-tight is-ok",
                        span { class: "stat-label", "可租面积" }
                        strong { class: "stat-value is-compact is-mono is-ok", "{format_area(record.available_area)} ㎡" }
                    }
                }

                // 出租率直接给出来，不必让人心算「总面积减已用面积」
                div { class: "stack-tight",
                    div { class: "section-header",
                        span { class: "field-label", "出租率" }
                        strong { class: "is-mono", "{occupancy:.1}%" }
                    }
                    Progress { value: occupancy, max: 100.0 }
                }

                Separator {}
                div { class: "copy",
                    h4 { "厂房描述" }
                    p { "{description}" }
                }
                FloorSection { factory_id, floors: record.floors.clone() }
                FacilitySection { record }
            }
        }
    }
}

#[component]
fn FloorSection(factory_id: u64, floors: Vec<FloorDetailRecord>) -> Element {
    let can_edit = can_edit_assets();
    let mut adding = use_signal(|| false);
    // 楼层改成磁贴网格 + 单个展开：原来每层一行折叠条，五层就把页面拉长
    // 五屏，而每行中间又全是留白。网格一屏看得完，详情只开当前这一层。
    let mut selected = use_signal(|| None::<u64>);
    let current = selected().and_then(|id| {
        floors
            .iter()
            .find(|record| record.floor.floor_id == id)
            .cloned()
    });

    rsx! {
        section { class: "subsection",
            header { class: "section-header",
                h4 { "楼层分布" }
                div { class: "section-tools",
                    Badge { variant: BadgeVariant::Outline, "{floors.len()} 层" }
                    if can_edit && !adding() {
                        Button { size: ButtonSize::Xs, r#type: "button", onclick: move |_| adding.set(true), "新增楼层" }
                    }
                }
            }
            if adding() {
                FloorInlineForm { factory_id, floor: None, previews: Vec::new(), on_done: move |_| adding.set(false) }
            }
            if floors.is_empty() && !adding() {
                p { class: "empty", "暂无楼层信息" }
            } else {
                div { class: "grid-4",
                    for record in floors.iter() {
                        {
                            let floor = record.floor.clone();
                            let floor_id = floor.floor_id;
                            let used_area = record.used_area;
                            let available = (floor.total_area_centi_square_metres - used_area).max(0);
                            let is_open = selected() == Some(floor_id);
                            rsx! {
                                button {
                                    key: "floor-tile-{floor_id}",
                                    class: "tile",
                                    r#type: "button",
                                    aria_pressed: is_open,
                                    onclick: move |_| {
                                        selected.set(if is_open { None } else { Some(floor_id) });
                                    },
                                    span { class: "tile-label", "{floor.floor_name}" }
                                    span { class: "is-mono", "{format_area(available)} ㎡" }
                                    Badge {
                                        variant: if available > 0 { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                        if available > 0 {
                                            "可招商"
                                        } else {
                                            "已满租"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(record) = current {
                    {
                        let floor = record.floor.clone();
                        let used_area = record.used_area;
                        let available = (floor.total_area_centi_square_metres - used_area).max(0);
                        let description = floor
                            .description
                            .clone()
                            .unwrap_or_else(|| "暂无楼层描述".into());
                        rsx! {
                            div { class: "panel is-plain",
                                div { class: "section-header",
                                    h4 { "{floor.floor_name}" }
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        size: ButtonSize::Xs,
                                        r#type: "button",
                                        onclick: move |_| selected.set(None),
                                        "收起"
                                    }
                                }
                                FloorRowActions { can_edit, floor: floor.clone() }
                                // 配图和正文并排：图片独占一行会把面积、租金推到首屏之外
                                div { class: "grid-media",
                                    ImageGallery {
                                        images: record.images.clone(),
                                        title: floor.floor_name.clone(),
                                        compact: true,
                                    }
                                    div { class: "stack",
                                        dl { class: "facts",
                                            div { dt { "层高" } dd { class: "is-mono", "{format_centi(floor.floor_height_centi_metres)} 米" } }
                                            div { dt { "承重" } dd { class: "is-mono", "{format_centi(floor.load_bearing_centi_units)} 吨/㎡" } }
                                            div { dt { "挂牌租金" } dd { class: "is-mono", "{format_money(Some(floor.rent_price_cents))} 元/㎡/月" } }
                                            div { dt { "总面积" } dd { class: "is-mono", "{format_area(floor.total_area_centi_square_metres)} ㎡" } }
                                            div { dt { "已用面积" } dd { class: "is-mono", "{format_area(used_area)} ㎡" } }
                                            div { dt { "可租面积" } dd { class: "is-mono", "{format_area(available)} ㎡" } }
                                        }
                                        p { class: "copy", "{description}" }
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

/// 楼层卡片内的编辑与删除入口。
#[component]
fn FloorRowActions(can_edit: bool, floor: FactoryFloor) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut editing = use_signal_sync(|| false);
    let mut confirming = use_signal_sync(|| false);
    let mut deleting = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let floor_id = floor.floor_id;

    if !can_edit {
        return rsx! {};
    }
    rsx! {
        div { class: "collapse-actions",
            if !editing() {
                Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, r#type: "button", onclick: move |_| editing.set(true), "编辑楼层" }
                Button { variant: ButtonVariant::Outline, class: "is-quiet-danger", size: ButtonSize::Sm, r#type: "button", onclick: move |_| confirming.set(true), "删除楼层" }
                if confirming() {
                    ConfirmDialog {
                        title: "删除楼层？",
                        description: "该楼层及其图片会被移除，园区可租面积将重新计算。",
                        confirm_label: "确认删除",
                        busy: deleting(),
                        error: error(),
                        on_cancel: move |_| confirming.set(false),
                        on_confirm: move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                if let Err(message) = delete_factory_floor_record(floor_id).await {
                                    error.set(Some(message));
                                }
                                deleting.set(false);
                                confirming.set(false);
                            });
                        },
                    }
                }
            }
        }
        if let Some(message) = error() { p { class: "form-error", "{message}" } }
        if editing() {
            FloorInlineForm {
                factory_id: floor.factory_id,
                floor: Some(floor.clone()),
                previews: (state.factory_floor_image_previews)()
                    .into_iter()
                    .filter(|preview| preview.floor_id == floor_id)
                    .collect::<Vec<_>>(),
                on_done: move |_| editing.set(false),
            }
        }
    }
}

#[component]
fn FacilitySection(record: FactoryDetailRecord) -> Element {
    let factory_name = record.factory.factory_name.clone();
    rsx! {
        section { class: "subsection",
            header { class: "section-header",
                h4 { "设施概览" }
                span { class: "hint", "数量与巡检状态" }
            }
            // 这三张卡是体检报告而不是设备清单：只回答「有多少、有没有问题」，
            // 逐台的规格参数在维护管理页看。设计依据见 园区管理.md §2.3.1。
            div { class: "grid-3",
                div {
                    FirefightingOverviewCard {
                        summary: record.firefighting,
                        factory_name: factory_name.clone(),
                    }
                }
                div {
                    DeviceOverviewCard {
                        title: "变压器".to_string(),
                        unit: "台".to_string(),
                        view: record.transformer,
                        target: FacilityTarget::Transformer,
                        factory_name: factory_name.clone(),
                    }
                }
                div {
                    DeviceOverviewCard {
                        title: "电梯".to_string(),
                        unit: "台".to_string(),
                        view: record.elevator,
                        target: FacilityTarget::Elevator,
                        factory_name,
                    }
                }
            }
        }
    }
}

/// 概览卡跳转到哪个维护管理页面。
#[derive(Clone, Copy, PartialEq)]
enum FacilityTarget {
    Firefighting,
    Transformer,
    Elevator,
}

impl FacilityTarget {
    /// 带上厂房名作为查询参数，落地即是这个厂房的设备，而不是全园区一大列。
    fn route(self, factory: String) -> crate::router::Route {
        use crate::router::Route;
        match self {
            Self::Firefighting => Route::MaintenanceFirefightingPage { factory },
            Self::Transformer => Route::MaintenanceTransformerPage { factory },
            Self::Elevator => Route::MaintenanceElevatorPage { factory },
        }
    }
}

/// 卡片底部的出口按钮。
///
/// 对没有维护菜单权限的账号同样显示：点过去会被路由守卫拦下并给出提示，
/// 这比直接隐藏好——隐藏了用户只会以为系统没有这个功能，显示了他至少知道
/// 该去哪、该找谁开权限。
#[component]
fn FacilityJumpButton(target: FacilityTarget, factory_name: String, label: String) -> Element {
    let navigator = navigator();
    rsx! {
        Button {
            variant: ButtonVariant::Outline,
            size: ButtonSize::Sm,
            r#type: "button",
            onclick: move |_| {
                navigator.push(target.route(factory_name.clone()));
            },
            "{label}"
        }
    }
}

/// 变压器、电梯：逐台点名，但列表有界（至多三台，异常优先）。
#[component]
fn DeviceOverviewCard(
    title: String,
    unit: String,
    view: Option<DeviceFacilityView>,
    target: FacilityTarget,
    factory_name: String,
) -> Element {
    rsx! {
        Card {
            CardContent {
                div { class: "stack",
                    if let Some(view) = view {
                        div { class: "facility-head",
                            h4 { "{title}" }
                            Badge { variant: BadgeVariant::Secondary, "{view.total} {unit}" }
                        }
                        if view.abnormal > 0 {
                            p { class: "facility-alert", "{view.abnormal} {unit}巡检异常" }
                        }
                        ul { class: "facility-list",
                            for entry in view.entries.iter() {
                                li { key: "{entry.name}", class: "facility-row",
                                    span { class: "facility-name", "{entry.name}" }
                                    Badge {
                                        variant: if entry.abnormal() { BadgeVariant::Destructive } else { BadgeVariant::Outline },
                                        "{entry.status_label()}"
                                    }
                                    span { class: "facility-date is-mono",
                                        {entry.checked_on.clone().unwrap_or_default()}
                                    }
                                }
                            }
                        }
                        if view.overflow() > 0 {
                            small { class: "hint", "还有 {view.overflow()} {unit}未列出" }
                        }
                        FacilityJumpButton {
                            target,
                            factory_name,
                            label: "查看并登记巡检".to_string(),
                        }
                    } else {
                        div { class: "facility-head",
                            h4 { "{title}" }
                        }
                        p { class: "hint", "该厂房还没有登记{title}" }
                        FacilityJumpButton {
                            target,
                            factory_name,
                            label: format!("去维护管理登记{title}"),
                        }
                    }
                }
            }
        }
    }
}

/// 消防设施：数量多的同类小件，按类型汇总而不是逐个罗列。
#[component]
fn FirefightingOverviewCard(
    summary: Option<FirefightingSummaryView>,
    factory_name: String,
) -> Element {
    rsx! {
        Card {
            CardContent {
                div { class: "stack",
                    if let Some(summary) = summary {
                        div { class: "facility-head",
                            h4 { "消防设施" }
                            Badge { variant: BadgeVariant::Secondary, "{summary.total} 个" }
                        }
                        p { class: "facility-types",
                            {
                                summary
                                    .by_type
                                    .iter()
                                    .map(|(name, count)| format!("{name} {count}"))
                                    .collect::<Vec<_>>()
                                    .join(" · ")
                            }
                        }
                        // 计数为零时整行不出现：一片零值会把真正的告警淹掉。
                        if summary.abnormal > 0 || summary.expired > 0 {
                            div { class: "row",
                                if summary.abnormal > 0 {
                                    span { class: "facility-alert", "巡检异常 {summary.abnormal} 个" }
                                }
                                if summary.expired > 0 {
                                    span { class: "facility-alert", "已过期 {summary.expired} 个" }
                                }
                            }
                        }
                        FacilityJumpButton {
                            target: FacilityTarget::Firefighting,
                            factory_name,
                            label: "查看并登记巡检".to_string(),
                        }
                    } else {
                        div { class: "facility-head",
                            h4 { "消防设施" }
                        }
                        p { class: "hint", "该厂房楼层还没有登记消防设施" }
                        FacilityJumpButton {
                            target: FacilityTarget::Firefighting,
                            factory_name,
                            label: "去维护管理登记消防设施".to_string(),
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn DormitorySection(park_id: u64, dormitories: Vec<DormitoryDetailRecord>) -> Element {
    let can_edit = can_edit_assets();
    // 楼层与合同占用直接从工作区状态读：房间总数在楼层上，已用间数由合同算。
    let workspace = use_context::<crate::state::WorkspaceState>();
    let dormitory_floors = (workspace.dormitory_floors)();
    let dormitory_links = (workspace.rental_tenant_dormitory_floors)();
    let mut adding = use_signal_sync(|| false);
    rsx! {
        div { class: "section",
            Card {
            CardHeader {
                div { class: "section-header",
                    CardTitle { "宿舍信息" }
                    div { class: "section-tools",
                    Badge { variant: BadgeVariant::Secondary, "{dormitories.len()} 个" }
                    if can_edit && !adding() {
                        Button { size: ButtonSize::Sm, r#type: "button", onclick: move |_| adding.set(true), "新增宿舍" }
                    }
                    }
                }
            }
            if adding() {
                DormitoryInlineForm { park_id, dormitory: None, previews: Vec::new(), on_done: move |_| adding.set(false) }
            }
            CardContent {
                if dormitories.is_empty() && !adding() {
                    p { class: "empty", "暂无宿舍信息" }
                } else {
                    div { class: "stack",
                    for (index, record) in dormitories.into_iter().enumerate() {
                        { let dormitory = record.dormitory.clone();
                          let remark = dormitory.remark.clone().unwrap_or_else(|| "暂无宿舍备注".into());
                          // 每层的房间总数固定，占用间数由合同关联算出，两者相减即为可租。
                          let floors = dormitory_floors.iter().filter(|f| f.dormitory_id == dormitory.dormitory_id).cloned().collect::<Vec<_>>();
                          let total_rooms: i32 = floors.iter().map(|f| f.room_count).sum();
                          let used_rooms: i32 = floors.iter().map(|f| occupied_rooms(f.dormitory_floor_id, &dormitory_links)).sum();
                          rsx! {
                            Collapsible { key: "dormitory-{dormitory.dormitory_id}", default_open: index < 3, CollapsibleTrigger {
                                    div { class: "collapse-summary",
                                        strong { "{dormitory.dormitory_name}" }
                                        span { "{used_rooms} / {total_rooms} 间已用" }
                                    }
                                }
                                CollapsibleContent { class: "collapse-body",
                                    DormitoryRowActions { can_edit, park_id, dormitory: dormitory.clone() }
                                    div { class: "grid-media",
                                    ImageGallery { images: record.images, title: dormitory.dormitory_name.clone(), compact: true }
                                    div { class: "stack",
                                        if can_edit {
                                            DormitoryFloorAdd { dormitory_id: dormitory.dormitory_id }
                                        }
                                        if floors.is_empty() {
                                            p { class: "hint", "尚未维护楼层。层高、房间数与挂牌租金都在楼层上填写。" }
                                        } else {
                                            table { class: "table",
                                                thead { tr {
                                                    th { "楼层" } th { "房间数" } th { "已用" } th { "可租" }
                                                    th { "单间面积" } th { "层高" } th { "挂牌租金" }
                                                    if can_edit { th { "操作" } }
                                                } }
                                                tbody {
                                                    for floor in floors.iter() {
                                                        {
                                                            let used = occupied_rooms(floor.dormitory_floor_id, &dormitory_links);
                                                            let free = floor.room_count - used;
                                                            rsx! {
                                                                tr { key: "dorm-floor-{floor.dormitory_floor_id}",
                                                                    td { "{floor.floor_no} 层" }
                                                                    td { class: "is-mono", "{floor.room_count}" }
                                                                    td { class: "is-mono", "{used}" }
                                                                    td { class: "is-mono", "{free}" }
                                                                    td { class: "is-mono", "{format_centi(floor.room_area_centi_square_metres)} ㎡" }
                                                                    td { class: "is-mono", "{format_centi(floor.floor_height_centi_metres)} 米" }
                                                                    td { class: "is-mono", "{format_money(floor.rent_price_cents)} 元/间/月" }
                                                                    if can_edit {
                                                                        td { DormitoryFloorRowActions { floor: floor.clone(), occupied: used } }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        p { class: "copy", "{remark}" }
                                    }
                                    }
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
}

/// 宿舍卡片内的编辑与删除入口。
#[component]
fn DormitoryRowActions(can_edit: bool, park_id: u64, dormitory: Dormitory) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut editing = use_signal_sync(|| false);
    let mut confirming = use_signal_sync(|| false);
    let mut deleting = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let dormitory_id = dormitory.dormitory_id;

    if !can_edit {
        return rsx! {};
    }
    rsx! {
        div { class: "collapse-actions",
            if !editing() {
                Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, r#type: "button", onclick: move |_| editing.set(true), "编辑宿舍" }
                Button { variant: ButtonVariant::Outline, class: "is-quiet-danger", size: ButtonSize::Sm, r#type: "button", onclick: move |_| confirming.set(true), "删除宿舍" }
                if confirming() {
                    ConfirmDialog {
                        title: "删除宿舍？",
                        description: "该宿舍及其图片会被归档，历史记录仍然保留。",
                        confirm_label: "确认删除",
                        busy: deleting(),
                        error: error(),
                        on_cancel: move |_| confirming.set(false),
                        on_confirm: move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                if let Err(message) = delete_dormitory_record(dormitory_id).await {
                                    error.set(Some(message));
                                }
                                deleting.set(false);
                                confirming.set(false);
                            });
                        },
                    }
                }
            }
        }
        if let Some(message) = error() { p { class: "form-error", "{message}" } }
        if editing() {
            DormitoryInlineForm {
                park_id,
                dormitory: Some(dormitory.clone()),
                previews: (state.dormitory_image_previews)()
                    .into_iter()
                    .filter(|preview| preview.dormitory_id == dormitory_id)
                    .collect::<Vec<_>>(),
                on_done: move |_| editing.set(false),
            }
        }
    }
}

/// 某个宿舍楼层被有效合同占用了多少间。
///
/// 这就是原来那个人工填写的「已用房间数」的替代品：房间总数固定在楼层上，
/// 占用数从合同关联算出来，两者相减即为可租。
fn occupied_rooms(
    dormitory_floor_id: u64,
    links: &[crate::spacetime_bindings::rental_tenant_dormitory_floor_type::RentalTenantDormitoryFloor],
) -> i32 {
    links
        .iter()
        .filter(|link| link.dormitory_floor_id == dormitory_floor_id)
        .map(|link| link.room_count)
        .sum()
}

/// 宿舍楼层的新增入口。
#[component]
fn DormitoryFloorAdd(dormitory_id: u64) -> Element {
    let mut adding = use_signal_sync(|| false);
    rsx! {
        if adding() {
            DormitoryFloorInlineForm {
                dormitory_id,
                floor: None,
                on_done: move |_| adding.set(false),
            }
        } else {
            div { class: "collapse-actions",
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Sm,
                    r#type: "button",
                    onclick: move |_| adding.set(true),
                    "新增楼层"
                }
            }
        }
    }
}

/// 单个宿舍楼层的编辑与删除。
///
/// 已被合同占用的楼层不允许直接删除——服务端也会挡，这里提前把原因说清楚，
/// 免得用户点了才知道。
#[component]
fn DormitoryFloorRowActions(
    floor: crate::spacetime_bindings::dormitory_floor_type::DormitoryFloor,
    occupied: i32,
) -> Element {
    let mut editing = use_signal_sync(|| false);
    let mut confirming = use_signal_sync(|| false);
    let mut deleting = use_signal_sync(|| false);
    let mut error = use_signal_sync(|| None::<String>);
    let floor_id = floor.dormitory_floor_id;

    rsx! {
        if editing() {
            DormitoryFloorInlineForm {
                dormitory_id: floor.dormitory_id,
                floor: Some(floor.clone()),
                on_done: move |_| editing.set(false),
            }
        } else {
            div { class: "row-tight",
                Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, r#type: "button", onclick: move |_| editing.set(true), "编辑" }
                if occupied == 0 {
                    Button { variant: ButtonVariant::Outline, class: "is-quiet-danger", size: ButtonSize::Sm, r#type: "button", onclick: move |_| confirming.set(true), "删除" }
                } else {
                    span { class: "hint", "{occupied} 间在租，不可删除" }
                }
                if confirming() {
                    ConfirmDialog {
                        title: "删除该楼层？",
                        description: "楼层会被归档，历史记录仍然保留。",
                        confirm_label: "确认删除",
                        busy: deleting(),
                        error: error(),
                        on_cancel: move |_| confirming.set(false),
                        on_confirm: move |_| {
                            deleting.set(true);
                            error.set(None);
                            spawn(async move {
                                match delete_dormitory_floor_record(floor_id).await {
                                    Ok(()) => { deleting.set(false); confirming.set(false); }
                                    Err(message) => { deleting.set(false); error.set(Some(message)); }
                                }
                            });
                        },
                    }
                }
            }
        }
    }
}
