//! 原系统“园区详情”独立路由页面。

use dioxus::prelude::*;

use super::{
    detail_model::{build_park_detail, format_datetime, format_optional_datetime},
    detail_sections::{DormitorySection, FactorySection, ImageGallery},
    meter_section::MeterSection,
    model::{format_area, status_is_enabled, status_label},
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::Button,
        card::{Card, CardContent, CardHeader, CardTitle},
        separator::Separator,
    },
    pages::rental_management::ParkProfileDialog,
    permissions::can_manage_rental,
    router::Route,
    state::{ConnectionPhase, WorkspaceState},
};

#[component]
pub fn RentalParkDetailPage(id: u64) -> Element {
    let state = use_context::<WorkspaceState>();
    let mut editing = use_signal_sync(|| false);
    // 详情页本身挂在园区列表菜单下，编辑权限需要单独判定，
    // 与服务端 `require_rental_manager` 保持一致，避免显示注定失败的按钮。
    let can_edit = can_manage_rental(&(state.roles)(), &(state.menus)());
    let detail = build_park_detail(
        id,
        &(state.parks)(),
        &(state.park_image_previews)(),
        &(state.factories)(),
        &(state.factory_floors)(),
        &(state.factory_floor_image_previews)(),
        &(state.dormitories)(),
        &(state.dormitory_floors)(),
        &(state.dormitory_image_previews)(),
        &(state.rental_tenants)(),
        &(state.rental_tenant_floors)(),
        &(state.firefighting_assets)(),
        &(state.firefighting_inspections)(),
        &(state.transformer_assets)(),
        &(state.transformer_inspections)(),
        &(state.elevator_assets)(),
        &(state.elevator_inspections)(),
        &crate::pages::today_date(),
    );

    let Some(detail) = detail else {
        let connected = (state.phase)() == ConnectionPhase::Connected;
        return rsx! {
            main { class: "page",
                Card {
                    CardHeader {
                        CardTitle { if connected { "未找到园区详情" } else { "正在同步园区详情" } }
                    }
                    CardContent {
                        p { if connected { "该园区不存在、已被删除，或当前账号没有查看权限。" } else { "正在等待 SpacetimeDB 推送园区及资产关系，请稍候。" } }
                        Link { to: Route::RentalManagementPage {}, "返回园区管理" }
                    }
                }
            }
        };
    };

    let park = detail.park.clone();
    let status = status_label(&park);
    let enabled = status_is_enabled(&park);
    let description = park
        .description
        .clone()
        .unwrap_or_else(|| "暂无园区描述".into());
    let created_at = format_datetime(park.created_at);
    let updated_at = format_optional_datetime(park.updated_at);
    // 面积不再人工填写：厂房按各层面积求和，宿舍按「房间数 × 单间面积」，
    // 两者分开显示——出租方式和计价单位都不一样，加在一起没有业务意义。
    let factory_area = format_area(detail.areas.factory);
    let dormitory_area = format_area(detail.areas.dormitory);
    let used_area = format_area(detail.used_area);
    let available_area = format_area(detail.available_area);
    let factory_count = detail.factories.len();
    let dormitory_count = detail.dormitories.len();

    rsx! {
        main { class: "page",
            header { class: "page-header",
                h1 { "园区详情" }
                div { class: "section-tools",
                    if can_edit {
                        Button { r#type: "button", onclick: move |_| editing.set(true), "编辑档案" }
                    }
                    Link { class: "hint", to: Route::RentalManagementPage {}, "← 返回园区管理" }
                }
            }

            if editing() {
                ParkProfileDialog {
                    park: Some(park.clone()),
                    previews: (state.park_image_previews)()
                        .into_iter()
                        .filter(|preview| preview.park_id == id)
                        .collect::<Vec<_>>(),
                    on_close: move |_| editing.set(false),
                    on_saved: move |_| editing.set(false),
                }
            }

            div { class: "section",
            Card {
                CardHeader {
                    div { class: "section-header",
                        CardTitle { "{park.park_name}" }
                        Badge {
                            variant: if enabled { BadgeVariant::Primary } else { BadgeVariant::Outline },
                            "{status}"
                        }
                    }
                }
                CardContent {
                    div { class: "grid-split",
                        div { class: "overview-media",
                            ImageGallery { images: detail.images.clone(), title: park.park_name.clone(), compact: false }
                        }
                        dl { class: "facts",
                            div { dt { "厂房面积" } dd { "{factory_area} ㎡" } }
                            div { dt { "宿舍面积" } dd { "{dormitory_area} ㎡" } }
                            div { dt { "厂房已租" } dd { "{used_area} ㎡" } }
                            div { dt { "厂房可租" } dd { "{available_area} ㎡" } }
                            div { dt { "厂房" } dd { "{factory_count} 个" } }
                            div { dt { "宿舍" } dd { "{dormitory_count} 个" } }
                            div { dt { "创建时间" } dd { "{created_at}" } }
                            div { class: "is-wide", dt { "地址" } dd { "{park.address}" } }
                            div { class: "is-wide", dt { "更新时间" } dd { "{updated_at}" } }
                        }
                    }
                    Separator {}
                    div { class: "copy",
                        h4 { "园区描述" }
                        p { "{description}" }
                    }
                }
            }
            }

            FactorySection { park_id: id, factories: detail.factories }
            DormitorySection { park_id: id, dormitories: detail.dormitories }
            MeterSection { park_id: id }
        }
    }
}
