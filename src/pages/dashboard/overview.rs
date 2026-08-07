//! 登录后的园区经营总览页面。

use dioxus::prelude::*;

use super::{
    models::{collection_rate, format_area, format_money},
    widgets::{AssetStat, MetricCard, RevenuePulseChart, TodoItem},
};
use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        card::{Card, CardContent, CardDescription, CardHeader, CardTitle},
        progress::Progress,
        separator::Separator,
        Icon,
    },
    state::WorkspaceState,
};

#[component]
pub fn DashboardPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let user = state.current_user.read().clone();
    let overview = state.dashboard_overview.read().clone();
    let first_name = user
        .as_ref()
        .map(|user| user.real_name.clone())
        .unwrap_or_else(|| "运营伙伴".into());
    let tenant_name = overview
        .as_ref()
        .map(|data| data.customer_id.clone())
        .or_else(|| user.as_ref().and_then(|user| user.customer_type.clone()))
        .unwrap_or_else(|| "公共空间".into());
    let receivable = overview.as_ref().map_or(0, |data| data.receivable_cents);
    let received = overview.as_ref().map_or(0, |data| data.received_cents);
    let outstanding = overview.as_ref().map_or(0, |data| data.outstanding_cents);
    let collection_rate = collection_rate(receivable, received);
    let menu_count = state.menus.read().len();
    #[cfg(target_arch = "wasm32")]
    let now_micros = (js_sys::Date::now() * 1_000.0).min(i64::MAX as f64) as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let now_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_micros() as i64)
        .unwrap_or_default();
    let expiry_limit_micros = now_micros.saturating_add(30 * 86_400_000_000);
    let contract_expiry_count = state
        .rental_tenants
        .read()
        .iter()
        .filter(|tenant| {
            !tenant.is_deleted
                && !matches!(
                    tenant.status.as_deref(),
                    Some("已退租") | Some("已终止") | Some("已取消")
                )
                && tenant.contract_end.is_some_and(|end| {
                    let end = end.to_micros_since_unix_epoch();
                    end >= now_micros && end <= expiry_limit_micros
                })
        })
        .count() as u64;
    // 空置判定改由合同占用算出：楼层总面积固定，减去合同占用即为可租。
    let floor_used = crate::pages::floor_used_areas(
        &(state.rental_tenant_floors)(),
        &(state.rental_tenants)(),
    );
    let vacant_factory_count = state
        .factory_floors
        .read()
        .iter()
        .filter(|floor| {
            !floor.is_deleted
                && floor.total_area_centi_square_metres
                    > floor_used.get(&floor.floor_id).copied().unwrap_or(0)
        })
        .count() as u64;
    let pending_repair_count = state
        .repair_orders
        .read()
        .iter()
        .filter(|order| !matches!(order.status.as_str(), "已完成" | "已取消"))
        .count() as u64;
    let attendance_abnormal_count = state.attendance_abnormal_logs.read().len() as u64;
    let outstanding_bill_count = overview
        .as_ref()
        .map_or(0, |data| data.outstanding_bill_count);
    let pending_reimbursement_count = overview
        .as_ref()
        .map_or(0, |data| data.pending_reimbursement_count);
    let todo_total = outstanding_bill_count
        .saturating_add(contract_expiry_count)
        .saturating_add(pending_reimbursement_count)
        .saturating_add(vacant_factory_count)
        .saturating_add(pending_repair_count)
        .saturating_add(attendance_abnormal_count);
    let collection_progress: ReadSignal<Option<f64>> =
        use_memo(move || Some(collection_rate as f64)).into();

    rsx! {        main { class: "page",
            header { class: "page-header",
                div {
                    h1 { "园区经营总览" }
                    p { class: "page-subtitle",
                        "{first_name}，这里汇总 {tenant_name} 的经营回款、资产状态与优先待办。"
                    }
                }
                div { class: "section-tools",
                    Badge { variant: BadgeVariant::Secondary, "统计空间 · {tenant_name}" }
                    Badge { variant: BadgeVariant::Outline, "数据口径 · 实时订阅" }
                    Badge { variant: BadgeVariant::Primary,
                        span { class: "live-pulse" }
                        "ONLINE"
                    }
                }
            }

            section { class: "grid-4", aria_label: "经营核心指标",
                MetricCard {
                    label: "累计应收",
                    value: format_money(receivable),
                    detail: "账单应收总额 · 元",
                    icon: "chart",
                }
                MetricCard {
                    label: "累计实收",
                    value: format_money(received),
                    detail: "回款率 {collection_rate}%",
                    icon: "database",
                }
                MetricCard {
                    label: "待收余额",
                    value: format_money(outstanding),
                    detail: "{outstanding_bill_count} 笔账单待跟进",
                    icon: "chart",
                }
                MetricCard {
                    label: "在租面积",
                    value: format_area(overview.as_ref().map_or(0, |data| data.occupied_area_centi_square_metres)),
                    detail: "{overview.as_ref().map_or(0, |data| data.tenant_count)} 家租赁客户",
                    icon: "building",
                }
            }

            section { class: "grid-split",
                Card {
                    CardHeader {
                        div { class: "section-header",
                            div {
                                CardTitle { "经营回款脉搏" }
                                CardDescription { "按账单创建月份对比应收与实收，数据变化会实时重绘。" }
                            }
                            div { class: "section-tools",
                                Badge { variant: BadgeVariant::Outline, "应收" }
                                Badge { variant: BadgeVariant::Secondary, "实收" }
                            }
                        }
                    }
                    CardContent {
                        RevenuePulseChart {
                            points: overview.as_ref().map(|data| data.revenue_pulse.clone()).unwrap_or_default()
                        }
                        Separator {}
                        div { class: "row",
                            div { class: "row",
                                span { "回款完成度" }
                                strong { "{collection_rate}%" }
                            }
                            Progress { value: collection_progress, aria_label: "回款完成度" }
                            Link { class: "hint", to: "/bill", "查看账单 →" }
                        }
                    }
                }

                Card {
                    CardHeader {
                        div { class: "section-header",
                            CardTitle { "待办事项" }
                            Badge { variant: BadgeVariant::Secondary, "共 {todo_total} 条" }
                        }
                    }
                    CardContent {
                        div { class: "list",
                            TodoItem {
                                title: "未收租提醒",
                                description: "存在余额的应收账单",
                                count: outstanding_bill_count,
                                path: "/bill",
                                icon: "chart",
                            }
                            TodoItem {
                                title: "合同到期提醒",
                                description: "30 天内到期合同",
                                count: contract_expiry_count,
                                path: "/rental/tenants",
                                icon: "database",
                            }
                            TodoItem {
                                title: "待处理报销",
                                description: "等待审批或复核",
                                count: pending_reimbursement_count,
                                path: "/reimbursement/audit",
                                icon: "database",
                            }
                            TodoItem {
                                title: "空置厂房",
                                description: "仍有可租面积的楼层",
                                count: vacant_factory_count,
                                path: "/rental/manage",
                                icon: "building",
                            }
                            TodoItem {
                                title: "维护工单",
                                description: "尚未完结的维修任务",
                                count: pending_repair_count,
                                path: "/maintenance/repair-order",
                                icon: "database",
                            }
                            TodoItem {
                                title: "考勤异常",
                                description: "设备与打卡异常记录",
                                count: attendance_abnormal_count,
                                path: "/hrm/attendance/stats",
                                icon: "chart",
                            }
                        }
                        p { class: "hint",
                            span { class: "live-pulse" }
                            "跨模块事项已按当前角色过滤"
                        }
                    }
                }
            }

            section { class: "grid-split is-even",
                Card {
                    CardHeader {
                        div { class: "section-header",
                            CardTitle { "资产与团队快照" }
                            Link { class: "hint", to: "/rental/factory", "进入资产台账 ↗" }
                        }
                    }
                    CardContent {
                        div { class: "grid-auto",
                            AssetStat {
                                label: "厂房资产",
                                value: overview.as_ref().map_or(0, |data| data.factory_count).to_string(),
                                caption: "处可见厂房",
                            }
                            AssetStat {
                                label: "租赁客户",
                                value: overview.as_ref().map_or(0, |data| data.tenant_count).to_string(),
                                caption: "家合同主体",
                            }
                            AssetStat {
                                label: "园区团队",
                                value: overview.as_ref().map_or(0, |data| data.employee_count).to_string(),
                                caption: "名在册成员",
                            }
                            AssetStat {
                                label: "业务模块",
                                value: menu_count.to_string(),
                                caption: "个已授权入口",
                            }
                        }
                    }
                }

                Card {
                    CardHeader {
                        div { class: "section-header",
                            CardTitle { "常用入口" }
                            Badge { variant: BadgeVariant::Outline, "{menu_count} 个" }
                        }
                    }
                    CardContent {
                        if menu_count == 0 {
                            p { class: "empty", "等待角色菜单授权" }
                        } else {
                            nav { class: "list", aria_label: "常用业务入口",
                                for menu in state.menus.read().iter().filter(|menu| !menu.path.is_empty()).take(4) {
                                    Link { class: "list-item", to: menu.path.clone(),
                                        span { class: "list-icon", Icon { name: "grid" } }
                                        span { class: "list-item-copy",
                                            strong { "{menu.name}" }
                                            small { "{menu.path}" }
                                        }
                                        span { class: "hint", "→" }
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
