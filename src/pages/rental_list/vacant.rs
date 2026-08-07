//! 原系统“租赁 / 待租厂房”的 Dioxus 页面。

use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        input::Input,
        label::Label,
        select::{Select, SelectOption},
        total_pages, Pager,
    },
    spacetime_bindings::{
        factory_floor_image_preview_type::FactoryFloorImagePreview,
        factory_floor_type::FactoryFloor, factory_type::Factory, park_type::Park,
    },
    state::{ConnectionPhase, WorkspaceState},
};

const PAGE_SIZE: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VacancyPriority {
    Normal,
    Warning,
    Urgent,
}

impl VacancyPriority {
    fn from_area(area_centi_square_metres: i64) -> Self {
        if area_centi_square_metres >= 500_000 {
            Self::Urgent
        } else if area_centi_square_metres >= 100_000 {
            Self::Warning
        } else {
            Self::Normal
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Urgent => "优先招商",
            Self::Warning => "重点关注",
            Self::Normal => "常规待租",
        }
    }

    fn filter_value(self) -> &'static str {
        match self {
            Self::Urgent => "urgent",
            Self::Warning => "warning",
            Self::Normal => "normal",
        }
    }

    /// 优先招商用实心徽章拉开权重，其余两档描边即可。
    fn badge_variant(self) -> BadgeVariant {
        match self {
            Self::Urgent => BadgeVariant::Secondary,
            _ => BadgeVariant::Outline,
        }
    }
}

#[derive(Clone, PartialEq)]
struct VacantFactoryRecord {
    factory: Factory,
    park_name: String,
    /// 厂房所在地址。取自园区——厂房不再单独维护地址，同一园区的厂房共用
    /// 一个地址，哪一栋由厂房名称区分。
    park_address: String,
    image_url: Option<String>,
    floor_count: usize,
    total_area_centi_square_metres: i64,
    available_area_centi_square_metres: i64,
    minimum_rent_price_cents: Option<i64>,
    priority: VacancyPriority,
}

#[component]
pub fn VacantFactoryPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let mut keyword = use_signal(String::new);
    let mut park_id = use_signal(String::new);
    let mut priority = use_signal(String::new);
    let mut page = use_signal(|| 1usize);
    let park_value: ReadSignal<Option<String>> = use_memo(move || Some(park_id())).into();
    let priority_value: ReadSignal<Option<String>> = use_memo(move || Some(priority())).into();

    let records = build_records(
        &(state.parks)(),
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.factory_floor_image_previews)(),
        &(state.rental_tenants)(),
        &(state.rental_tenant_floors)(),
    );
    let park_options = records
        .iter()
        .map(|row| (row.factory.park_id, row.park_name.clone()))
        .collect::<BTreeSet<_>>();
    let keyword_query = keyword().trim().to_lowercase();
    let park_query = park_id().parse::<u64>().ok();
    let priority_query = priority();
    let filtered = records
        .iter()
        .filter(|row| {
            (keyword_query.is_empty()
                || row
                    .factory
                    .factory_name
                    .to_lowercase()
                    .contains(&keyword_query)
                || row.park_address.to_lowercase().contains(&keyword_query)
                || row.park_name.to_lowercase().contains(&keyword_query))
                && park_query.is_none_or(|id| row.factory.park_id == id)
                && (priority_query.is_empty()
                    || row.priority.filter_value() == priority_query.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    let total = filtered.len();
    let page_count = total_pages(total, PAGE_SIZE);
    // 筛选收窄会让页数变少，越界时回落到最后一页，否则列表显示空白。
    let visible = filtered
        .into_iter()
        .skip((page().clamp(1, page_count) - 1) * PAGE_SIZE)
        .take(PAGE_SIZE)
        .collect::<Vec<_>>();
    let loading = (state.phase)() != ConnectionPhase::Connected && records.is_empty();
    let total_available = records
        .iter()
        .map(|row| row.available_area_centi_square_metres)
        .sum::<i64>();
    let urgent_count = records
        .iter()
        .filter(|row| row.priority == VacancyPriority::Urgent)
        .count();
    let park_count = records
        .iter()
        .map(|row| row.factory.park_id)
        .collect::<BTreeSet<_>>()
        .len();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "待租厂房" }
                    p { class: "page-subtitle",
                        "聚合当前权限范围内仍有可租面积的厂房，按招商优先级与空置面积实时排序。"
                    }
                }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待租厂房" }
                                strong { class: "stat-value is-mono", "{records.len()}" }
                                span { class: "stat-caption", "处" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "覆盖园区" }
                                strong { class: "stat-value is-mono", "{park_count}" }
                                span { class: "stat-caption", "个" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "优先招商" }
                                strong { class: "stat-value is-mono", "{urgent_count}" }
                                span { class: "stat-caption", "处 · 可租面积 5000 ㎡ 以上" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "可租总面积" }
                                strong { class: "stat-value is-compact is-mono is-ok",
                                    "{format_area(total_available)} ㎡"
                                }
                                span { class: "stat-caption", "全部待租厂房合计" }
                            }
                        }
                    }
                }
            }

            section { class: "section", aria_label: "筛选待租厂房",
                Card {
                    CardContent {
                        div { class: "filters",
                            div { class: "field",
                                Label { html_for: "vacant-keyword", "搜索厂房" }
                                Input {
                                    id: "vacant-keyword",
                                    value: keyword,
                                    placeholder: "输入厂房、园区或地址",
                                    oninput: move |event: FormEvent| {
                                        keyword.set(event.value());
                                        page.set(1);
                                    },
                                }
                            }
                            div { class: "field",
                                Label { html_for: "vacant-park", "所属园区" }
                                Select {
                                    id: "vacant-park",
                                    value: Some(park_value),
                                    on_value_change: move |value: Option<String>| {
                                        park_id.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部园区".to_string(), "全部园区" }
                                    for (index , (id , name)) in park_options.iter().enumerate() {
                                        SelectOption::<String> {
                                            key: "park-{id}",
                                            value: id.to_string(),
                                            index: index + 1,
                                            text_value: name.to_string(), "{name}"
                                        }
                                    }
                                }
                            }
                            div { class: "field",
                                Label { html_for: "vacant-priority", "招商优先级" }
                                Select {
                                    id: "vacant-priority",
                                    value: Some(priority_value),
                                    on_value_change: move |value: Option<String>| {
                                        priority.set(value.unwrap_or_default());
                                        page.set(1);
                                    },
                                    SelectOption::<String> { value: String::new(), index: 0usize, text_value: "全部优先级".to_string(), "全部优先级" }
                                    SelectOption::<String> { value: "urgent".to_string(), index: 1usize, text_value: "优先招商".to_string(), "优先招商" }
                                    SelectOption::<String> { value: "warning".to_string(), index: 2usize, text_value: "重点关注".to_string(), "重点关注" }
                                    SelectOption::<String> { value: "normal".to_string(), index: 3usize, text_value: "常规待租".to_string(), "常规待租" }
                                }
                            }
                            div { class: "field",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| {
                                        keyword.set(String::new());
                                        park_id.set(String::new());
                                        priority.set(String::new());
                                        page.set(1);
                                    },
                                    "重置筛选"
                                }
                            }
                        }
                    }
                }
            }

            if loading {
                p { class: "empty", aria_live: "polite",
                    "正在同步厂房与楼层数据，可租面积将在租赁资产订阅完成后自动计算。"
                }
            } else if visible.is_empty() {
                p { class: "empty",
                    if records.is_empty() {
                        "当前没有待租厂房。厂房楼层的总面积已经全部出租，或尚未维护楼层面积。"
                    } else {
                        "没有符合条件的厂房。请调整厂房名称、园区或招商优先级后再试。"
                    }
                }
            } else {
                section { class: "grid-3",
                    for record in visible {
                        {
                            let factory_id = record.factory.factory_id;
                            let description = record
                                .factory
                                .description
                                .as_deref()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .unwrap_or("暂无厂房补充说明");
                            rsx! {
                                div { key: "vacant-{factory_id}",
                                    Card {
                                        CardContent {
                                            div { class: "stack",
                                                div { class: "card-media",
                                                    if let Some(image_url) = record.image_url.as_deref() {
                                                        img { src: "{image_url}", alt: "{record.factory.factory_name}", loading: "lazy" }
                                                    } else {
                                                        span { class: "card-media-placeholder", "暂无图片" }
                                                    }
                                                }
                                                div { class: "row",
                                                    Badge { variant: record.priority.badge_variant(), "{record.priority.label()}" }
                                                    span { class: "hint", "{record.park_name}" }
                                                }
                                                div { class: "stack-tight",
                                                    h3 { "{record.factory.factory_name}" }
                                                    p { class: "hint", "{record.park_address}" }
                                                }
                                                // 可租面积是这一页的主信息，单独拎出来放大
                                                div { class: "panel is-tight is-ok",
                                                    span { class: "stat-label", "可租面积" }
                                                    strong { class: "stat-value is-compact is-mono is-ok",
                                                        "{format_area(record.available_area_centi_square_metres)} ㎡"
                                                    }
                                                }
                                                dl { class: "facts",
                                                    div {
                                                        dt { "总面积" }
                                                        dd { class: "is-mono", "{format_area(record.total_area_centi_square_metres)} ㎡" }
                                                    }
                                                    div {
                                                        dt { "楼层" }
                                                        dd { class: "is-mono", "{record.floor_count} 层" }
                                                    }
                                                    div {
                                                        dt { "最低挂牌租金" }
                                                        dd { class: "is-mono", "{format_rent(record.minimum_rent_price_cents)}" }
                                                    }
                                                }
                                                p { class: "line-clamp-2", "{description}" }
                                                div { class: "card-cta",
                                                    Link { to: crate::router::Route::VacantFactoryDetailPage { id: factory_id },
                                                        Button { variant: ButtonVariant::Outline, "查看厂房详情" }
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
                Pager { page, total_pages: page_count, total_count: total }
            }
        }
    }
}

fn build_records(
    parks: &[Park],
    factories: &[Factory],
    floors: &[FactoryFloor],
    image_previews: &[FactoryFloorImagePreview],
    tenants: &[crate::spacetime_bindings::rental_tenant_type::RentalTenant],
    tenant_floors: &[crate::spacetime_bindings::rental_tenant_floor_type::RentalTenantFloor],
) -> Vec<VacantFactoryRecord> {
    // 可租面积 = 楼层总面积 - 合同占用，占用由关联表算出而非人工维护。
    let floor_used = crate::pages::floor_used_areas(tenant_floors, tenants);
    let park_profiles = parks
        .iter()
        .filter(|park| !park.is_deleted)
        .map(|park| (park.park_id, (park.park_name.clone(), park.address.clone())))
        .collect::<BTreeMap<_, _>>();
    let first_image =
        image_previews
            .iter()
            .fold(BTreeMap::<u64, String>::new(), |mut images, preview| {
                images
                    .entry(preview.floor_id)
                    .or_insert_with(|| preview.img_url.clone());
                images
            });

    let mut records = factories
        .iter()
        .filter(|factory| !factory.is_deleted)
        .filter_map(|factory| {
            let factory_floors = floors
                .iter()
                .filter(|floor| floor.factory_id == factory.factory_id && !floor.is_deleted)
                .collect::<Vec<_>>();
            let total_area = factory_floors
                .iter()
                .map(|floor| floor.total_area_centi_square_metres.max(0))
                .sum::<i64>();
            let available_area = factory_floors
                .iter()
                .map(|floor| {
                    let used = floor_used.get(&floor.floor_id).copied().unwrap_or(0);
                    (floor.total_area_centi_square_metres - used).max(0)
                })
                .sum::<i64>();
            if available_area <= 0 {
                return None;
            }
            let minimum_rent = factory_floors
                .iter()
                .filter(|floor| {
                    floor.total_area_centi_square_metres
                        > floor_used.get(&floor.floor_id).copied().unwrap_or(0)
                        && floor.rent_price_cents > 0
                })
                .map(|floor| floor.rent_price_cents)
                .min();
            let image_url = factory_floors
                .iter()
                .find_map(|floor| first_image.get(&floor.floor_id).cloned());
            Some(VacantFactoryRecord {
                factory: factory.clone(),
                park_name: park_profiles
                    .get(&factory.park_id)
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| "未分配园区".into()),
                park_address: park_profiles
                    .get(&factory.park_id)
                    .map(|(_, address)| address.clone())
                    .unwrap_or_default(),
                image_url,
                floor_count: factory_floors.len(),
                total_area_centi_square_metres: total_area,
                available_area_centi_square_metres: available_area,
                minimum_rent_price_cents: minimum_rent,
                priority: VacancyPriority::from_area(available_area),
            })
        })
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| {
                right
                    .available_area_centi_square_metres
                    .cmp(&left.available_area_centi_square_metres)
            })
            .then_with(|| left.factory.factory_id.cmp(&right.factory.factory_id))
    });
    records
}

fn format_area(value: i64) -> String {
    let value = value.max(0);
    if value % 100 == 0 {
        (value / 100).to_string()
    } else {
        format!("{}.{:02}", value / 100, value % 100)
            .trim_end_matches('0')
            .to_string()
    }
}

fn format_rent(value: Option<i64>) -> String {
    value
        .map(|cents| {
            if cents % 100 == 0 {
                format!("¥{}/㎡", cents / 100)
            } else {
                format!("¥{}.{:02}/㎡", cents / 100, cents % 100)
            }
        })
        .unwrap_or_else(|| "面议".into())
}
