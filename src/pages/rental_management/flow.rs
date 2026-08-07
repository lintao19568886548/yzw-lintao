//! 租赁模块的数据流向总览。
//!
//! 这里刻意画成两条独立的链路，而不是一条"园区→厂房→楼层→客户→合同"的
//! 直线——数据库里真的是这样：`RentalTenant`（合同）只有 `park_id`，
//! 没有指向厂房或楼层的字段，合同是直接挂在园区下的，压根不经过厂房、
//! 楼层。硬画成一条线会暗示一个不存在的因果关系。

use dioxus::prelude::*;

use super::model::{
    factory_park_link_counts, floor_factory_link_counts, floor_occupancy_counts,
    is_active_income_contract, unmatched_contract_count,
};
use crate::{
    components::{
        card::{Card, CardContent},
        data_flow::DataFlowStage,
        Icon,
    },
    permissions::can_access_route,
    router::Route,
    state::WorkspaceState,
};

#[component]
pub fn RentalOverviewPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();

    let parks = (state.parks)();
    let factories = (state.factories)();
    let floors = (state.factory_floors)();
    let tenant_profiles = (state.tenant_profiles)();
    let rental_tenants = (state.rental_tenants)();

    let park_count = parks.iter().filter(|park| !park.is_deleted).count();
    let (factory_linked, factory_orphaned) = factory_park_link_counts(&factories, &parks);
    let factory_count = factory_linked + factory_orphaned;
    let (floor_linked, floor_orphaned) = floor_factory_link_counts(&floors, &factories);
    let floor_used = crate::pages::floor_used_areas(
        &(state.rental_tenant_floors)(),
        &rental_tenants,
    );
    let (full_floors, vacant_floors) = floor_occupancy_counts(&floors, &floor_used);
    let floor_count = floor_linked + floor_orphaned;
    let contract_count = rental_tenants
        .iter()
        .filter(|row| is_active_income_contract(row))
        .count();
    let unmatched_contracts = unmatched_contract_count(&rental_tenants, &tenant_profiles);
    let matched_contracts = contract_count.saturating_sub(unmatched_contracts);

    let manage_link = can_access_route(&roles, &menus, "/rental/manage")
        .then_some(Route::RentalManagementPage {});
    let factory_link =
        can_access_route(&roles, &menus, "/rental/factory").then_some(Route::VacantFactoryPage {});
    let tenant_link = can_access_route(&roles, &menus, "/rental/tenants")
        .then_some(Route::TenantManagementPage {});
    let contract_link = can_access_route(&roles, &menus, "/rental/tenant")
        .then_some(Route::ContractManagementPage {});

    let max_count = [park_count, factory_count, floor_count, contract_count, matched_contracts]
        .into_iter()
        .max()
        .unwrap_or(0)
        .max(1);

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "租赁总览" }
                    p { class: "page-subtitle",
                        "两条各自独立的数据链路：园区名下的物理资产，和园区名下的合同与客户。两条链路只在“园区”这一点上交汇，彼此之间没有直接关联。"
                    }
                }
            }
            section { class: "section",
                Card {
                    CardContent {
                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title",
                                "空间资产：园区 → 厂房 → 楼层（都是数据库外键，一路可追溯）"
                            }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "园区",
                                    caption: format!("{park_count} 个在管园区"),
                                    count: park_count,
                                    max_count,
                                    to: manage_link.clone(),
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "厂房",
                                    caption: format!("{factory_linked} 挂靠园区 · {factory_orphaned} 未挂靠"),
                                    count: factory_count,
                                    max_count,
                                    to: factory_link.clone(),
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "楼层",
                                    caption: format!("{floor_linked} 挂靠有效厂房 · {floor_orphaned} 归属已删厂房"),
                                    count: floor_count,
                                    max_count,
                                    to: factory_link,
                                    div { class: "data-flow-split",
                                        span { class: "data-flow-pill is-ok", "{full_floors} 满租" }
                                        span { class: "data-flow-pill", "{vacant_floors} 待招商" }
                                    }
                                }
                            }
                        }

                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title",
                                "合同与客户：合同直接挂在园区下（外键），跟客户档案之间靠姓名+电话匹配，不是外键"
                            }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "园区",
                                    caption: format!("{park_count} 个在管园区"),
                                    count: park_count,
                                    max_count,
                                    to: manage_link,
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "合同",
                                    caption: format!("{contract_count} 份有效合同 · 直接归属园区"),
                                    count: contract_count,
                                    max_count,
                                    to: contract_link,
                                }
                                div { class: "data-flow-arrow is-inferred", title: "姓名+电话文本匹配，数据库里没有这条外键",
                                    Icon { name: "link".to_string() }
                                }
                                DataFlowStage {
                                    label: "在租客户",
                                    caption: format!("{matched_contracts} 份合同匹配到客户档案 · {unmatched_contracts} 份未归档"),
                                    count: matched_contracts,
                                    max_count,
                                    to: tenant_link,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
