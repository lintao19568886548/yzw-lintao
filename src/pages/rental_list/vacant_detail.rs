//! 待租厂房的独立详情页，不复用园区详情页面。

use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
    },
    spacetime_bindings::{
        factory_floor_image_preview_type::FactoryFloorImagePreview,
        factory_floor_type::FactoryFloor,
    },
    state::{ConnectionPhase, WorkspaceState},
};

#[derive(Clone, PartialEq)]
struct FloorDetailRecord {
    floor: FactoryFloor,
    image_urls: Vec<String>,
    /// 这一层被有效合同占用的面积，由合同↔楼层关联算出。
    used_area: i64,
}

impl FloorDetailRecord {
    fn available_area(&self) -> i64 {
        (self.floor.total_area_centi_square_metres - self.used_area).max(0)
    }
}

#[component]
pub fn VacantFactoryDetailPage(id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let factory = (state.factories)()
        .into_iter()
        .find(|factory| factory.factory_id == id && !factory.is_deleted);

    let Some(factory) = factory else {
        let connected = (state.phase)() == ConnectionPhase::Connected;
        return rsx! {
            main { class: "page",
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { if connected { "未找到厂房" } else { "正在同步厂房资料" } }
                            p { class: "page-subtitle",
                                if connected {
                                    "该厂房不存在、已删除或不在当前账号权限范围内。"
                                } else {
                                    "实时数据就绪后将自动显示厂房详情。"
                                }
                            }
                            div { class: "card-cta",
                                Link { to: crate::router::Route::VacantFactoryPage {},
                                    Button { variant: ButtonVariant::Outline, "返回待租厂房" }
                                }
                            }
                        }
                    }
                }
            }
        };
    };

    // 地址跟着园区走：厂房不再单独维护地址，同一园区的厂房共用一个地址，
    // 具体哪一栋由厂房名称区分。
    let park = (state.parks)()
        .into_iter()
        .find(|park| park.park_id == factory.park_id && !park.is_deleted);
    let park_address = park
        .as_ref()
        .map(|park| park.address.clone())
        .unwrap_or_default();
    let park_name = park
        .map(|park| park.park_name)
        .unwrap_or_else(|| "未分配园区".into());
    let images_by_floor = image_map(&(state.factory_floor_image_previews)());
    let floor_used = crate::pages::floor_used_areas(
        &(state.rental_tenant_floors)(),
        &(state.rental_tenants)(),
    );
    let mut floors = (state.factory_floors)()
        .into_iter()
        .filter(|floor| floor.factory_id == factory.factory_id && !floor.is_deleted)
        .map(|floor| FloorDetailRecord {
            image_urls: images_by_floor
                .get(&floor.floor_id)
                .cloned()
                .unwrap_or_default(),
            used_area: floor_used.get(&floor.floor_id).copied().unwrap_or(0),
            floor,
        })
        .collect::<Vec<_>>();
    floors.sort_by_key(|record| record.floor.floor_id);

    let total_area = floors
        .iter()
        .map(|record| record.floor.total_area_centi_square_metres.max(0))
        .sum::<i64>();
    let available_area = floors
        .iter()
        .map(FloorDetailRecord::available_area)
        .sum::<i64>();
    let minimum_rent = floors
        .iter()
        .filter(|record| record.available_area() > 0 && record.floor.rent_price_cents > 0)
        .map(|record| record.floor.rent_price_cents)
        .min();
    let gallery = floors
        .iter()
        .flat_map(|record| record.image_urls.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let available_floor_count = floors
        .iter()
        .filter(|record| record.available_area() > 0)
        .count();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    Link { class: "hint", to: crate::router::Route::VacantFactoryPage {}, "← 返回待租厂房" }
                    h1 { "{factory.factory_name}" }
                    p { class: "page-subtitle", "{park_name} · {park_address}" }
                }
                div { class: "page-actions",
                    Badge { variant: BadgeVariant::Secondary,
                        "可租 {format_area(available_area)} ㎡"
                    }
                }
            }

            section { class: "grid-4",
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "总面积" }
                                strong { class: "stat-value is-compact is-mono", "{format_area(total_area)} ㎡" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "可租面积" }
                                strong { class: "stat-value is-compact is-mono is-ok",
                                    "{format_area(available_area)} ㎡"
                                }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "待租楼层" }
                                strong { class: "stat-value is-mono", "{available_floor_count} / {floors.len()}" }
                            }
                        }
                    }
                }
                div {
                    Card {
                        CardContent {
                            div { class: "stat",
                                span { class: "stat-label", "最低挂牌租金" }
                                strong { class: "stat-value is-compact is-mono", "{format_rent(minimum_rent)}" }
                            }
                        }
                    }
                }
            }

            section { class: "grid-split",
                div {
                    Card {
                        CardContent {
                            div { class: "stack",
                                h2 { "现场图片" }
                                if gallery.is_empty() {
                                    p { class: "empty", "该厂房暂未上传现场图片。" }
                                } else {
                                    // 缩略图网格：原来第一张做大图、其余平铺，
                                    // 在宽屏上第一张会拉到上千像素高。
                                    div { class: "grid-3",
                                        for (index , url) in gallery.iter().take(6).enumerate() {
                                            div { key: "gallery-{index}", class: "card-media",
                                                img {
                                                    src: "{url}",
                                                    alt: "{factory.factory_name} 现场图片 {index + 1}",
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
                div {
                    Card {
                        CardContent {
                            div { class: "stack",
                                h2 { "厂房资料" }
                                dl { class: "facts",
                                    div {
                                        dt { "所属园区" }
                                        dd { "{park_name}" }
                                    }
                                    div {
                                        dt { "建成日期" }
                                        dd { class: "is-mono",
                                            {factory.build_date.clone().unwrap_or_else(|| "未维护".to_string())}
                                        }
                                    }
                                    div {
                                        dt { "产权类型" }
                                        dd {
                                            if factory.is_own {
                                                "自有厂房"
                                            } else {
                                                "入驻厂房"
                                            }
                                        }
                                    }
                                    div { class: "is-wide",
                                        dt { "园区地址" }
                                        dd { "{park_address}" }
                                    }
                                    div { class: "is-wide",
                                        dt { "厂房说明" }
                                        dd {
                                            {
                                                factory
                                                    .description
                                                    .clone()
                                                    .filter(|value| !value.trim().is_empty())
                                                    .unwrap_or_else(|| "暂无厂房说明".to_string())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "section",
                div { class: "section-header",
                    h2 { "楼层台账" }
                    Badge { variant: BadgeVariant::Outline, "{available_floor_count} / {floors.len()} 层待租" }
                }
                if floors.is_empty() {
                    p { class: "empty", "该厂房尚未维护楼层资料。" }
                } else {
                    // 楼层是逐层对比面积与租金的场景，表格比卡片更好扫
                    div { class: "table-shell",
                        table { class: "table",
                            thead {
                                tr {
                                    th { "楼层" }
                                    th { "状态" }
                                    th { "总面积(㎡)" }
                                    th { "已使用(㎡)" }
                                    th { "可租(㎡)" }
                                    th { "挂牌租金" }
                                    th { "层高" }
                                    th { "承重" }
                                    th { "说明" }
                                }
                            }
                            tbody {
                                for record in floors {
                                    {
                                        let available = record.available_area();
                                        let description = record
                                            .floor
                                            .description
                                            .clone()
                                            .filter(|value| !value.trim().is_empty())
                                            .unwrap_or_else(|| "—".to_string());
                                        rsx! {
                                            tr { key: "floor-{record.floor.floor_id}",
                                                td {
                                                    strong { "{record.floor.floor_name}" }
                                                }
                                                td {
                                                    Badge {
                                                        variant: if available > 0 { BadgeVariant::Secondary } else { BadgeVariant::Outline },
                                                        if available > 0 {
                                                            "待租"
                                                        } else {
                                                            "已满"
                                                        }
                                                    }
                                                }
                                                td { class: "is-mono", "{format_area(record.floor.total_area_centi_square_metres)}" }
                                                td { class: "is-mono", "{format_area(record.used_area)}" }
                                                td { class: if available > 0 { "is-mono is-ok" } else { "is-mono" },
                                                    "{format_area(available)}"
                                                }
                                                td { class: "is-mono", "{format_rent_value(record.floor.rent_price_cents)}" }
                                                td { class: "is-mono",
                                                    {format_optional_decimal(record.floor.floor_height_centi_metres, "m")}
                                                }
                                                td { class: "is-mono",
                                                    {format_optional_decimal(record.floor.load_bearing_centi_units, "")}
                                                }
                                                td { class: "is-wrap hint", "{description}" }
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

fn image_map(rows: &[FactoryFloorImagePreview]) -> BTreeMap<u64, Vec<String>> {
    let mut result = BTreeMap::<u64, Vec<String>>::new();
    for row in rows {
        let urls = result.entry(row.floor_id).or_default();
        if !urls.contains(&row.img_url) {
            urls.push(row.img_url.clone());
        }
    }
    result
}

fn format_area(value: i64) -> String {
    format!("{:.2}", value.max(0) as f64 / 100.0)
}

fn format_rent(value: Option<i64>) -> String {
    value
        .map(format_rent_value)
        .unwrap_or_else(|| "面议".into())
}

fn format_rent_value(value: i64) -> String {
    if value <= 0 {
        "面议".into()
    } else {
        format!("¥{:.2}/㎡", value as f64 / 100.0)
    }
}

fn format_optional_decimal(value: Option<i64>, unit: &str) -> String {
    value
        .map(|value| format!("{:.2}{unit}", value as f64 / 100.0))
        .unwrap_or_else(|| "未维护".into())
}

fn display_text(value: &str) -> &str {
    if value.trim().is_empty() {
        "未维护"
    } else {
        value
    }
}
