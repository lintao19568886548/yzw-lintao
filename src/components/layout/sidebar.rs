//! 根据 SpacetimeDB `my_menus` 视图生成的动态侧边栏。

use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;

use crate::{
    components::{
        sidebar::{
            use_sidebar, Sidebar as LibSidebar, SidebarCollapsible, SidebarContent, SidebarFooter,
            SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader, SidebarMenu,
            SidebarMenuBadge, SidebarMenuButton, SidebarMenuItem, SidebarMenuSub,
            SidebarMenuSubButton, SidebarMenuSubItem, SidebarRail,
        },
        BrandLogo, Icon, LogoSize,
    },
    permissions::{can_access_route, can_manage_hr, can_manage_permissions, can_manage_rental},
    services::logout_workspace,
    spacetime_bindings::menu_type::Menu,
    state::WorkspaceState,
};

#[derive(Clone, PartialEq)]
struct MenuNode {
    menu: Menu,
    children: Vec<MenuNode>,
}

#[derive(Clone, Copy)]
struct MobileDrawer(Signal<bool>);

fn build_menu_nodes(menus: &[Menu]) -> Vec<MenuNode> {
    let visible = menus
        .iter()
        .filter(|menu| {
            menu.status == 1
                && menu.template_deleted_at.is_none()
                && !menu.template_internal_only
                // 已迁移的静态页面由下方权限入口渲染，避免和动态菜单重复。
                && !matches!(
                    menu.path.as_str(),
                    "/"
                        | "/rental"
                        | "/system/park"
                        | "/rental/manage"
                        | "/rental/list"
                        | "/rental/factory"
                        | "/rental/tenant"
                        | "/rental/salary"
                        | "/finance"
                        | "/finance/manage"
                        | "/bill"
                        | "/bill/carryover"
                        | "/reimbursement/application"
                        | "/reimbursement/audit"
                        | "/hrm"
                        | "/smart-meter"
                        | "/data-collection"
                        | "/maintenance"
                        | "/access"
                        | "/crm"
                        | "/crm/qrcode-test"
                        | "/system"
                        | "/system/role"
                )
                && !menu.path.starts_with("/hrm/")
                && menu.path != "/smart-meter/meter"
                && menu.path != "/smart-meter/water"
                && !menu.path.starts_with("/device/")
                && !menu.path.starts_with("/maintenance/")
                && !menu.path.starts_with("/access/")
        })
        .cloned()
        .collect::<Vec<_>>();
    let ids = visible
        .iter()
        .map(|menu| menu.menu_id)
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<Option<u64>, Vec<Menu>>::new();
    for menu in visible {
        let parent = menu.parent_id.filter(|parent_id| ids.contains(parent_id));
        children.entry(parent).or_default().push(menu);
    }
    for rows in children.values_mut() {
        rows.sort_by_key(|menu| menu.menu_id);
    }

    fn assemble(
        parent_id: Option<u64>,
        children: &BTreeMap<Option<u64>, Vec<Menu>>,
        visiting: &mut BTreeSet<u64>,
    ) -> Vec<MenuNode> {
        children
            .get(&parent_id)
            .into_iter()
            .flatten()
            .filter_map(|menu| {
                if !visiting.insert(menu.menu_id) {
                    return None;
                }
                let nested = assemble(Some(menu.menu_id), children, visiting);
                visiting.remove(&menu.menu_id);
                Some(MenuNode {
                    menu: menu.clone(),
                    children: nested,
                })
            })
            .collect()
    }

    assemble(None, &children, &mut BTreeSet::new())
}

fn icon_for_path(path: &str) -> &'static str {
    if path.contains("customer") || path.contains("crm") || path.contains("user") {
        "users"
    } else if path.contains("park") || path.contains("factory") || path.contains("tenant") {
        "building"
    } else if path.contains("meter") {
        "meter"
    } else if path.contains("analytics")
        || path.contains("finance")
        || path.contains("bill")
        || path.contains("salary")
    {
        "chart"
    } else if path.contains("system") || path.contains("setting") {
        "settings"
    } else {
        "grid"
    }
}

#[component]
fn MenuBranch(node: MenuNode, current_path: String, depth: usize) -> Element {
    let ctx = use_sidebar();
    let active = current_path == node.menu.path
        || node
            .menu
            .active_path
            .as_ref()
            .is_some_and(|path| path == &current_path);
    let has_children = !node.children.is_empty();
    let path = node.menu.path.clone();
    let name = node.menu.name.clone();
    let icon = icon_for_path(&node.menu.path);

    // 一级用 SidebarMenuButton，下级用 SidebarMenuSubButton——和静态菜单
    // 共用同一套组件，缩进和高亮才一致。
    if depth == 0 {
        return rsx! {
            SidebarMenuItem {
                SidebarMenuButton {
                    is_active: active,
                    tooltip: rsx! { "{name}" },
                    as: move |attrs: Vec<Attribute>| {
                        let path = path.clone();
                        let name = name.clone();
                        rsx! {
                            Link { to: path, attributes: attrs,
                                Icon { name: icon }
                                span { "{name}" }
                            }
                        }
                    },
                }
                if has_children {
                    SidebarMenuSub {
                        for child in node.children.clone() {
                            MenuBranch {
                                key: "{child.menu.menu_id}",
                                node: child,
                                current_path: current_path.clone(),
                                depth: depth + 1,
                            }
                        }
                    }
                }
            }
        };
    }

    rsx! {
        SidebarMenuSubItem {
            SidebarMenuSubButton {
                is_active: active,
                as: move |attrs: Vec<Attribute>| {
                    let path = path.clone();
                    let name = name.clone();
                    rsx! {
                        Link {
                            to: path,
                            onclick: move |_| ctx.set_open_mobile(false),
                            attributes: attrs,
                            span { "{name}" }
                        }
                    }
                },
            }
        }
        if has_children {
            for child in node.children.clone() {
                MenuBranch {
                    key: "{child.menu.menu_id}",
                    node: child,
                    current_path: current_path.clone(),
                    depth: depth + 1,
                }
            }
        }
    }
}

/// 维护模块使用程序化导航，避免部分浏览器把 `Link` 退化为普通页面跳转后，
/// 地址栏已经变化但 Dioxus Router 仍停留在旧路由。
#[component]
fn MaintenanceNavButton(
    route: crate::router::Route,
    current_path: String,
    title: String,
    icon: String,
) -> Element {
    let navigator = navigator();
    let MobileDrawer(mut mobile_open) = use_context::<MobileDrawer>();
    let target_path = crate::router::route_path(&route);
    let active = current_path == target_path;

    rsx! {
        button {
            r#type: "button",
            class: if active { "sidebar-link sidebar-route-button is-active" } else { "sidebar-link sidebar-route-button" },
            title: "{title}",
            onclick: move |_| {
                mobile_open.set(false);
                navigator.push(route.clone());
            },
            Icon { name: icon }
            span { class: "sidebar-link-label", "{title}" }
        }
    }
}

#[component]
pub fn AppSidebar() -> Element {
    let state = use_context::<WorkspaceState>();
    let route = use_route::<crate::router::Route>();
    let current_path = crate::router::route_path(&route);
    let device_route_active =
        current_path.starts_with("/smart-meter/") || current_path.starts_with("/device/");
    let access_route_active = current_path.starts_with("/access/");
    let finance_route_active = current_path == "/finance/overview"
        || current_path == "/finance/manage"
        || current_path == "/bill"
        || current_path == "/reimbursement/application"
        || current_path == "/reimbursement/audit";
    let personnel_route_active = current_path.starts_with("/hrm/")
        || current_path == "/rental/salary"
        || current_path == "/system/role";
    let rental_route_active = matches!(
        current_path.as_str(),
        "/rental/overview" | "/rental/manage" | "/rental/factory" | "/rental/tenant"
            | "/rental/tenants"
    ) || current_path.starts_with("/rental/detail/")
        || current_path.starts_with("/rental/factory/detail/");
    let maintenance_route_active = current_path.starts_with("/maintenance/");
    let mut device_expanded = use_signal(move || device_route_active);
    let mut access_expanded = use_signal(move || access_route_active);
    let mut finance_expanded = use_signal(move || finance_route_active);
    let mut personnel_expanded = use_signal(move || personnel_route_active);
    let mut rental_expanded = use_signal(move || rental_route_active);
    let mut maintenance_expanded = use_signal(move || maintenance_route_active);
    let mut logout_pending = use_signal(|| false);
    let user = state.current_user.read().clone();
    // 财务分组的待办数：未收账单 + 待处理报销，收起成图标栏时也能看见。
    let finance_pending = state
        .dashboard_overview
        .read()
        .as_ref()
        .map(|overview| overview.outstanding_bill_count + overview.pending_reimbursement_count)
        .unwrap_or(0);
    let (
        nodes,
        can_view_dashboard,
        can_view_data_map,
        can_view_rental,
        can_view_park_assets,
        can_view_contract,
        can_view_smart_meter,
        smart_meter_target,
        can_view_device_overview,
        can_view_device_camera,
        can_view_device_access,
        can_view_device_gateway,
        can_view_salary,
        can_view_finance,
        can_view_bill,
        can_view_carryover,
        can_view_reimbursement,
        can_view_reimbursement_audit,
        can_view_hr,
        hr_target,
        can_view_access_car,
        can_view_access_visitor,
        can_view_access_register,
        can_view_firefighting,
        can_view_transformer,
        can_view_elevator,
        can_view_repair_order,
        can_manage_permissions,
    ) = {
        let roles = state.roles.read();
        let menus = state.menus.read();
        let permission_codes = state.permission_codes.read();
        let can_view_smart_electric = can_access_route(&roles, &menus, "/smart-meter/meter");
        let can_view_smart_water = can_access_route(&roles, &menus, "/smart-meter/water");
        let can_view_hr_information = can_access_route(&roles, &menus, "/hrm/information");
        let can_view_hr_punch = can_access_route(&roles, &menus, "/hrm/attendance/punch");
        let can_view_hr_records = can_access_route(&roles, &menus, "/hrm/attendance/stats");
        let can_view_hr_trajectory = can_access_route(&roles, &menus, "/hrm/trajectory");
        let can_view_hr_leave = can_access_route(&roles, &menus, "/hrm/leaveapplication");
        let can_audit_by_role = roles.iter().any(|role| {
            role.status == 1
                && ((role.name == "Super" && role.scope == "system")
                    || role.reimbursement_auth.unwrap_or(0) > 0)
        });
        (
            build_menu_nodes(&menus),
            can_access_route(&roles, &menus, "/"),
            can_access_route(&roles, &menus, "/data-map"),
            can_access_route(&roles, &menus, "/rental/manage") && can_manage_rental(&roles, &menus),
            // 待租厂房沿用后端原有的 /rental/list 菜单授权：园区列表页已删除，
            // 但菜单行仍在库里，改成别的路径会让现有角色失去访问权。
            can_access_route(&roles, &menus, "/rental/list"),
            can_access_route(&roles, &menus, "/rental/tenant"),
            can_view_smart_electric || can_view_smart_water,
            if can_view_smart_electric {
                crate::router::Route::SmartElectricMeterPage {}
            } else {
                crate::router::Route::SmartWaterMeterPage {}
            },
            can_access_route(&roles, &menus, "/device/overview"),
            can_access_route(&roles, &menus, "/device/camera"),
            can_access_route(&roles, &menus, "/device/access"),
            can_access_route(&roles, &menus, "/device/gateway"),
            can_access_route(&roles, &menus, "/rental/salary"),
            can_access_route(&roles, &menus, "/finance/manage"),
            can_access_route(&roles, &menus, "/bill"),
            can_access_route(&roles, &menus, "/bill/carryover"),
            can_access_route(&roles, &menus, "/reimbursement/application"),
            can_access_route(&roles, &menus, "/reimbursement/audit") && can_audit_by_role,
            can_view_hr_information
                || can_view_hr_punch
                || can_view_hr_records
                || can_view_hr_trajectory
                || can_view_hr_leave,
            if can_view_hr_information && can_manage_hr(&roles, &permission_codes) {
                crate::router::Route::HrmEmployeePage {}
            } else if can_view_hr_punch {
                crate::router::Route::HrmAttendancePunchPage {}
            } else if can_view_hr_records {
                crate::router::Route::HrmAttendanceRecordsPage {}
            } else if can_view_hr_trajectory {
                crate::router::Route::HrmTrajectoryPage {}
            } else {
                crate::router::Route::HrmLeavePage {}
            },
            can_access_route(&roles, &menus, "/access/car"),
            can_access_route(&roles, &menus, "/access/visitor"),
            can_access_route(&roles, &menus, "/access/visitor/register"),
            can_access_route(&roles, &menus, "/maintenance/firefighting"),
            can_access_route(&roles, &menus, "/maintenance/transformer"),
            can_access_route(&roles, &menus, "/maintenance/elevator"),
            can_access_route(&roles, &menus, "/maintenance/repair-order"),
            can_access_route(&roles, &menus, "/system/role")
                && can_manage_permissions(&roles, &menus),
        )
    };
    // 工作台只承载经营总览，具体业务能力统一进入各自一级菜单。
    let has_workbench_items = can_view_dashboard || can_view_data_map;
    let has_access_items =
        can_view_access_car || can_view_access_visitor || can_view_access_register;
    let has_finance_items = can_view_finance
        || can_view_bill
        || can_view_carryover
        || can_view_reimbursement
        || can_view_reimbursement_audit;
    let has_personnel_items = can_view_hr || can_view_salary || can_manage_permissions;
    let has_rental_items = can_view_rental || can_view_park_assets || can_view_contract;
    let has_maintenance_items =
        can_view_firefighting || can_view_transformer || can_view_elevator || can_view_repair_order;
    let has_device_items = can_view_smart_meter
        || can_view_device_overview
        || can_view_device_camera
        || can_view_device_access
        || can_view_device_gateway;
    let has_business_items = has_device_items
        || has_rental_items
        || has_personnel_items
        || has_finance_items
        || has_maintenance_items
        || has_access_items
        || !nodes.is_empty();
    let tenant = user
        .as_ref()
        .and_then(|user| user.customer_type.clone())
        .unwrap_or_else(|| "公共空间".into());

    rsx! {
        LibSidebar { collapsible: SidebarCollapsible::Icon,
            SidebarHeader {
                div { class: "brand",
                    BrandLogo { size: LogoSize::Small }
                    div { class: "brand-copy",
                        strong { "云园慧控" }
                        small { "{tenant}" }
                    }
                }
            }
            SidebarContent {
                if has_workbench_items {
                    SidebarGroup {
                        SidebarGroupLabel { "工作台" }
                        SidebarGroupContent {
                            SidebarMenu {
                                if can_view_dashboard {
                                    NavItem {
                                        to: crate::router::Route::DashboardPage {},
                                        icon: "grid".to_string(),
                                        label: "运营总览".to_string(),
                                        active: current_path == "/",
                                    }
                                }
                                if can_view_data_map {
                                    NavItem {
                                        to: crate::router::Route::DataMapPage {},
                                        icon: "database".to_string(),
                                        label: "数据地图".to_string(),
                                        active: current_path == "/data-map",
                                    }
                                }
                            }
                        }
                    }
                }
                if has_business_items {
                    SidebarGroup {
                        SidebarGroupLabel { "业务模块" }
                        SidebarGroupContent {
                            SidebarMenu {
                                if has_device_items {
                                    NavGroup { icon: "device".to_string(), label: "设备管理".to_string(), active: device_route_active, expanded: device_expanded,
                                        if can_view_device_overview {
                                            NavSubItem { to: crate::router::Route::DeviceOverviewPage {}, label: "设备总览".to_string(), active: current_path == "/device/overview" }
                                        }
                                        if can_view_smart_meter {
                                            NavSubItem { to: smart_meter_target, label: "智能水电表管理".to_string(), active: current_path.starts_with("/smart-meter/") }
                                        }
                                        if can_view_device_camera {
                                            NavSubItem { to: crate::router::Route::DeviceCameraPage {}, label: "摄像头管理".to_string(), active: current_path == "/device/camera" }
                                        }
                                        if can_view_device_access {
                                            NavSubItem { to: crate::router::Route::DeviceAccessPage {}, label: "门禁设备管理".to_string(), active: current_path == "/device/access" }
                                        }
                                        if can_view_device_gateway {
                                            NavSubItem { to: crate::router::Route::DeviceGatewayPage {}, label: "边缘计算设备".to_string(), active: current_path == "/device/gateway" }
                                        }
                                    }
                                }
                                if has_rental_items {
                                    NavGroup { icon: "building".to_string(), label: "租赁".to_string(), active: rental_route_active, expanded: rental_expanded, overview: Some(crate::router::Route::RentalOverviewPage {}), on_overview: current_path == "/rental/overview",
                                        if can_view_rental {
                                            NavSubItem { to: crate::router::Route::RentalManagementPage {}, label: "园区管理".to_string(), active: current_path == "/rental/manage" }
                                        }
                                        if can_view_park_assets {
                                            NavSubItem { to: crate::router::Route::VacantFactoryPage {}, label: "待租厂房".to_string(), active: current_path == "/rental/factory" || current_path.starts_with("/rental/factory/detail/") }
                                        }
                                        if can_view_contract {
                                            NavSubItem { to: crate::router::Route::TenantManagementPage {}, label: "租户管理".to_string(), active: current_path == "/rental/tenants" }
                                            NavSubItem { to: crate::router::Route::ContractManagementPage {}, label: "合同管理".to_string(), active: current_path == "/rental/tenant" }
                                        }
                                    }
                                }
                                if has_personnel_items {
                                    NavGroup { icon: "users".to_string(), label: "人事".to_string(), active: personnel_route_active, expanded: personnel_expanded, overview: Some(crate::router::Route::HrOverviewPage {}), on_overview: current_path == "/hrm/overview",
                                        if can_view_hr {
                                            NavSubItem { to: hr_target, label: "人事管理".to_string(), active: current_path.starts_with("/hrm/") && current_path != "/hrm/overview" }
                                        }
                                        if can_view_salary {
                                            NavSubItem { to: crate::router::Route::SalaryManagementPage {}, label: "工资管理".to_string(), active: current_path == "/rental/salary" }
                                        }
                                        if can_manage_permissions {
                                            NavSubItem { to: crate::router::Route::RoleManagementPage {}, label: "角色管理".to_string(), active: current_path == "/system/role" }
                                        }
                                    }
                                }
                                if has_finance_items {
                                    NavGroup {
                                        icon: "chart".to_string(),
                                        label: "财务".to_string(),
                                        active: finance_route_active,
                                        expanded: finance_expanded,
                                        badge: finance_pending,
                                        overview: Some(crate::router::Route::FinanceOverviewPage {}),
                                        on_overview: current_path == "/finance/overview",
                                        if can_view_finance {
                                            NavSubItem { to: crate::router::Route::FinanceManagementPage {}, label: "财务管理".to_string(), active: current_path == "/finance/manage" }
                                        }
                                        if can_view_bill {
                                            NavSubItem { to: crate::router::Route::BillManagementPage {}, label: "账单管理".to_string(), active: current_path == "/bill" }
                                        }
                                        if can_view_carryover {
                                            NavSubItem { to: crate::router::Route::BillCarryoverPage {}, label: "账期结转".to_string(), active: current_path == "/bill/carryover" }
                                        }
                                        if can_view_reimbursement {
                                            NavSubItem { to: crate::router::Route::ReimbursementApplicationPage {}, label: "报销管理".to_string(), active: current_path == "/reimbursement/application" }
                                        }
                                        if can_view_reimbursement_audit {
                                            NavSubItem { to: crate::router::Route::ReimbursementAuditPage {}, label: "报销审核".to_string(), active: current_path == "/reimbursement/audit" }
                                        }
                                    }
                                }
                                if has_access_items {
                                    NavGroup { icon: "lock".to_string(), label: "门禁管理".to_string(), active: access_route_active, expanded: access_expanded,
                                        if can_view_access_car {
                                            NavSubItem { to: crate::router::Route::AccessCarPage {}, label: "车辆出入管理".to_string(), active: current_path == "/access/car" }
                                        }
                                        if can_view_access_visitor {
                                            NavSubItem { to: crate::router::Route::AccessVisitorPage {}, label: "访客管理".to_string(), active: current_path == "/access/visitor" }
                                        }
                                        if can_view_access_register {
                                            NavSubItem { to: crate::router::Route::AccessVisitorRegisterPage {}, label: "访客登记".to_string(), active: current_path == "/access/visitor/register" }
                                        }
                                    }
                                }
                                if has_maintenance_items {
                                    NavGroup { icon: "tools".to_string(), label: "维护管理".to_string(), active: maintenance_route_active, expanded: maintenance_expanded,
                                        if can_view_firefighting {
                                            NavSubItem { to: crate::router::Route::MaintenanceFirefightingPage { factory: String::new() }, label: "消防管理".to_string(), active: current_path == "/maintenance/firefighting" }
                                        }
                                        if can_view_transformer {
                                            NavSubItem { to: crate::router::Route::MaintenanceTransformerPage { factory: String::new() }, label: "变压器管理".to_string(), active: current_path == "/maintenance/transformer" }
                                        }
                                        if can_view_elevator {
                                            NavSubItem { to: crate::router::Route::MaintenanceElevatorPage { factory: String::new() }, label: "电梯管理".to_string(), active: current_path == "/maintenance/elevator" }
                                        }
                                        if can_view_repair_order {
                                            NavSubItem { to: crate::router::Route::MaintenanceRepairOrderPage {}, label: "报修工单".to_string(), active: current_path == "/maintenance/repair-order" }
                                        }
                                    }
                                }
                                for node in nodes {
                                    MenuBranch { node, current_path: current_path.clone(), depth: 0 }
                                }
                            }
                        }
                    }
                }
            }
            SidebarFooter {
                div { class: "sidebar-foot-note", "数据同步 · {(state.last_synced_label)()}" }
                SidebarMenu {
                    SidebarMenuItem {
                        SidebarMenuButton {
                            tooltip: rsx! { "个人中心" },
                            as: move |attrs: Vec<Attribute>| {
                                let user = (state.business_user)();
                                let display_name = user
                                    .as_ref()
                                    .map(|user| user.real_name.clone())
                                    .filter(|name| !name.trim().is_empty())
                                    .unwrap_or_else(|| "个人中心".into());
                                let avatar = user.and_then(|user| user.avatar_url);
                                rsx! {
                                    button {
                                        r#type: "button",
                                        onclick: move |_| {
                                            navigator().push(crate::router::Route::ProfilePage {});
                                        },
                                        ..attrs,
                                        if let Some(url) = avatar {
                                            img { class: "sidebar-avatar", src: "{url}", alt: "头像" }
                                        } else {
                                            Icon { name: "user" }
                                        }
                                        span { "{display_name} · 个人中心" }
                                    }
                                }
                            },
                        }
                    }
                    SidebarMenuItem {
                        SidebarMenuButton {
                            tooltip: rsx! { "退出当前身份" },
                            as: move |attrs: Vec<Attribute>| rsx! {
                                button {
                                    r#type: "button",
                                    disabled: logout_pending(),
                                    onclick: move |_| {
                                        if logout_pending() {
                                            return;
                                        }
                                        logout_pending.set(true);
                                        if let Err(error) = logout_workspace(state) {
                                            logout_pending.set(false);
                                            let mut callback_state = state;
                                            callback_state.error_message.set(Some(error));
                                        }
                                    },
                                    ..attrs,
                                    Icon { name: "logout" }
                                    span {
                                        if logout_pending() { "正在退出…" } else { "退出当前身份" }
                                    }
                                }
                            },
                        }
                    }
                }
            }
            SidebarRail {}
        }
    }
}

/// 侧边栏一级导航项。
#[component]
fn NavItem(to: crate::router::Route, icon: String, label: String, active: bool) -> Element {
    let ctx = use_sidebar();
    rsx! {
        SidebarMenuItem {
            SidebarMenuButton {
                is_active: active,
                tooltip: rsx! { "{label}" },
                as: move |attrs: Vec<Attribute>| {
                    let to = to.clone();
                    let icon = icon.clone();
                    let label = label.clone();
                    rsx! {
                        Link { to, onclick: move |_| ctx.set_open_mobile(false), attributes: attrs,
                            Icon { name: icon }
                            span { "{label}" }
                        }
                    }
                },
            }
        }
    }
}

/// 可展开的一级分组。
#[component]
fn NavGroup(
    icon: String,
    label: String,
    active: bool,
    mut expanded: Signal<bool>,
    #[props(default = 0)] badge: u64,
    /// 一级菜单自己的落脚页（模块总览）。还没做总览页的模块传 `None`，
    /// 点击行为退回成纯粹的展开/收起——不强迫所有模块一次性迁移。
    #[props(default)]
    overview: Option<crate::router::Route>,
    /// 当前是否已经就在总览页本身。总览页不再作为二级列表里的一项出现——
    /// 点一级标签这个动作本身就是"去总览"，只有已经在总览页时再点一次
    /// 才退化成单纯展开/收起。
    #[props(default = false)]
    on_overview: bool,
    children: Element,
) -> Element {
    let navigator = navigator();
    rsx! {
        SidebarMenuItem {
            SidebarMenuButton {
                is_active: active,
                tooltip: rsx! { "{label}" },
                as: move |attrs: Vec<Attribute>| {
                    let icon = icon.clone();
                    let label = label.clone();
                    let overview = overview.clone();
                    rsx! {
                        button {
                            r#type: "button",
                            aria_expanded: if expanded() { "true" } else { "false" },
                            onclick: move |_| {
                                match overview.clone() {
                                    // 还没在总览页本身：点标签直接落到总览页，并展开子菜单。
                                    Some(route) if !on_overview => {
                                        navigator.push(route);
                                        expanded.set(true);
                                    }
                                    // 已经在总览页：再点一次退回普通的展开/收起开关，
                                    // 不然用户没法收起已经打开的子菜单。
                                    _ => expanded.set(!expanded()),
                                }
                            },
                            ..attrs,
                            Icon { name: icon }
                            span { "{label}" }
                        }
                    }
                },
            }
            if badge > 0 {
                SidebarMenuBadge { "{badge}" }
            }
            if expanded() {
                SidebarMenuSub { {children} }
            }
        }
    }
}

/// 分组内的二级导航项。
#[component]
fn NavSubItem(to: crate::router::Route, label: String, active: bool) -> Element {
    let ctx = use_sidebar();
    rsx! {
        SidebarMenuSubItem {
            SidebarMenuSubButton {
                is_active: active,
                as: move |attrs: Vec<Attribute>| {
                    let to = to.clone();
                    let label = label.clone();
                    rsx! {
                        Link { to, onclick: move |_| ctx.set_open_mobile(false), attributes: attrs, "{label}" }
                    }
                },
            }
        }
    }
}
