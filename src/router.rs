//! 静态外壳路由和动态业务页面入口。

use dioxus::prelude::*;

use crate::{
    components::WorkspaceLayout,
    components::{
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
        BrandLogo, LogoSize,
    },
    pages::{
        AccessCarPage, AccessVisitorPage, AccessVisitorRegisterPage, BillCarryoverPage,
        BillManagementPage,
        ContractCreatePage, ContractDetailPage, ContractEditPage, ContractManagementPage,
        DashboardPage, DataMapPage,
        DeviceAccessPage, DeviceCameraPage, DeviceGatewayPage, DeviceOverviewPage,
        DynamicWorkspacePage, FinanceManagementPage, FinanceOverviewPage, HrOverviewPage,
        HrmAttendanceLocationPage, HrmAttendancePunchPage,
        HrmAttendanceRecordsPage, HrmEmployeePage, HrmLeavePage, HrmTrajectoryPage,
        LoginPage, MaintenanceElevatorPage, MaintenanceFirefightingPage,
        MaintenanceInspectElevatorPage, MaintenanceInspectFirefightingPage,
        MaintenanceInspectTransformerPage, MaintenanceRepairOrderPage, MaintenanceTransformerPage,
        ProfilePage, ReimbursementApplicationPage,
        ReimbursementAuditPage, RentalManagementPage, RentalOverviewPage, RentalParkDetailPage,
        RoleManagementPage,
        SalaryManagementPage, SmartElectricMeterPage, SmartWaterMeterPage, TenantManagementPage,
        VacantFactoryDetailPage, VacantFactoryPage,
    },
    permissions::{can_access_route, first_accessible_path},
    services::activate_data_scope,
    state::{AuthState, DataScope, WorkspaceState},
};

#[cfg(test)]
thread_local! {
    /// 仅供路由渲染测试触发一次真实的 `Navigator::push`。
    static TEST_NAVIGATION_TARGET: std::cell::RefCell<Option<String>> = const {
        std::cell::RefCell::new(None)
    };
}

/// 路由的纯路径部分，剥掉 `?查询串`。
///
/// 菜单权限与侧边栏高亮都靠路径字符串比对，带上查询串会一律匹配失败。
pub(crate) fn route_path(route: &Route) -> String {
    let full = route.to_string();
    full.split('?').next().unwrap_or_default().to_string()
}

/// 恢复登录态、断线重连期间的占位屏。
///
/// 沿用登录页的品牌卡片，避免和登录表单之间跳变。
#[component]
fn AuthRestoringScreen(#[props(default = "正在恢复登录状态…")] message: &'static str) -> Element {
    rsx! {
        main { class: "auth",
            div { class: "auth-card",
                header { class: "auth-brand",
                    BrandLogo { size: LogoSize::Large }
                    div { class: "brand-copy",
                        strong { "云园慧控" }
                        small { "园区运营工作台" }
                    }
                }
                p { class: "hint", "{message}" }
            }
        }
    }
}

#[component]
fn WorkspaceGate() -> Element {
    let state = use_context::<WorkspaceState>();
    // 路由 Hook 必须在登录状态分支之前调用，保证每次渲染顺序稳定。
    let route = use_route::<Route>();
    let path = route_path(&route);
    let auth_state = (state.auth_state)();
    let data_scope = DataScope::for_path(&path);
    use_effect(use_reactive!(|(auth_state, data_scope)| {
        if auth_state == AuthState::SignedIn {
            if let Err(error) = activate_data_scope(data_scope, state) {
                let mut callback_state = state;
                callback_state.error_message.set(Some(error));
            }
        }
    }));
    #[cfg(test)]
    use_effect(move || {
        if let Some(target) = TEST_NAVIGATION_TARGET.with(|value| value.borrow_mut().take()) {
            if let Ok(route) = target.parse::<Route>() {
                navigator().push(route);
            }
        }
    });
    // 登录态还在恢复时不要直接甩出登录表单。会话有效的情况下它只会闪现一两秒
    // 然后被工作台替换掉，但用户每次打开都先看见一次登录页，观感上就等于「又要
    // 重新登录」，还可能已经开始输账号了。
    //
    // Checking 一定会有结果：订阅成功走 refresh_user，连接或订阅失败的分支都会
    // 落到 SignedOut（见 services/spacetime.rs），不会卡在这一屏。
    if auth_state == AuthState::Checking {
        return rsx! { AuthRestoringScreen {} };
    }
    if auth_state != AuthState::SignedIn {
        return rsx! { LoginPage {} };
    }
    // 断线重连时 connect_workspace 会清空所有缓存再重新订阅，这中间 current_user、
    // roles、menus 全是空的。此时不能去判权限——空菜单必然判定为无权访问，工作台
    // 会闪出「当前账号无权访问此页面」，而实际上只是数据还没回来。
    if state.current_user.read().is_none() {
        return rsx! { AuthRestoringScreen { message: "正在重新连接实时服务…" } };
    }
    let roles = state.roles.read();
    let menus = state.menus.read();
    if can_access_route(&roles, &menus, &path) {
        return rsx! { WorkspaceLayout {} };
    }
    let fallback = first_accessible_path(&roles, &menus);
    rsx! {
        main { class: "page",
            Card {
                CardContent {
                    div { class: "stack",
                        h1 { "当前账号无权访问此页面" }
                        p { class: "page-subtitle",
                            "路径 {path} 不在该账号的实时菜单权限中。即使直接输入网址，系统也不会加载对应业务页面。"
                        }
                        div { class: "card-cta",
                            a { href: "{fallback}",
                                Button { variant: ButtonVariant::Outline, "返回有权限的工作台" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(WorkspaceGate)]
        #[route("/")]
        DashboardPage {},
        #[route("/data-map")]
        DataMapPage {},
        #[route("/rental/salary")]
        SalaryManagementPage {},
        #[route("/rental/tenant")]
        ContractManagementPage {},
        #[route("/rental/tenant/new")]
        ContractCreatePage {},
        #[route("/rental/tenant/edit/:id")]
        ContractEditPage { id: u64 },
        #[route("/rental/tenant/detail/:id")]
        ContractDetailPage { id: u64 },
        #[route("/rental/tenants")]
        TenantManagementPage {},
        #[route("/rental/overview")]
        RentalOverviewPage {},
        #[route("/rental/manage")]
        RentalManagementPage {},
        #[route("/rental/factory")]
        VacantFactoryPage {},
        #[route("/rental/factory/detail/:id")]
        VacantFactoryDetailPage { id: u64 },
        #[route("/rental/detail/:id")]
        RentalParkDetailPage { id: u64 },
        #[route("/bill")]
        BillManagementPage {},
        #[route("/bill/carryover")]
        BillCarryoverPage {},
        #[route("/finance/overview")]
        FinanceOverviewPage {},
        #[route("/finance/manage")]
        FinanceManagementPage {},
        #[route("/reimbursement/application")]
        ReimbursementApplicationPage {},
        #[route("/reimbursement/audit")]
        ReimbursementAuditPage {},
        #[route("/hrm/overview")]
        HrOverviewPage {},
        #[route("/hrm/information")]
        HrmEmployeePage {},
        #[route("/hrm/attendance/punch")]
        HrmAttendancePunchPage {},
        #[route("/hrm/attendance/location")]
        HrmAttendanceLocationPage {},
        #[route("/hrm/attendance/stats")]
        HrmAttendanceRecordsPage {},
        #[route("/hrm/trajectory")]
        HrmTrajectoryPage {},
        #[route("/hrm/leaveapplication")]
        HrmLeavePage {},
        #[route("/smart-meter/meter")]
        SmartElectricMeterPage {},
        #[route("/smart-meter/water")]
        SmartWaterMeterPage {},
        #[route("/device/overview")]
        DeviceOverviewPage {},
        #[route("/device/camera")]
        DeviceCameraPage {},
        #[route("/device/access")]
        DeviceAccessPage {},
        #[route("/device/gateway")]
        DeviceGatewayPage {},
        // 查询参数用于从园区档案页跳转时预填厂房筛选（园区管理.md §2.3.1）。
        // 注意：带查询段的路由 to_string() 会附上 `?factory=`，凡是拿路径去匹配
        // 菜单权限或高亮侧边栏的地方，都必须先经 `route_path` 剥掉查询串。
        #[route("/maintenance/firefighting?:factory")]
        MaintenanceFirefightingPage { factory: String },
        #[route("/maintenance/transformer?:factory")]
        MaintenanceTransformerPage { factory: String },
        #[route("/maintenance/elevator?:factory")]
        MaintenanceElevatorPage { factory: String },
        #[route("/maintenance/repair-order")]
        MaintenanceRepairOrderPage {},
        #[route("/maintenance/inspect/transformer/:id")]
        MaintenanceInspectTransformerPage { id: u64 },
        #[route("/maintenance/inspect/elevator/:id")]
        MaintenanceInspectElevatorPage { id: u64 },
        #[route("/maintenance/inspect/firefighting/:id")]
        MaintenanceInspectFirefightingPage { id: u64 },
        #[route("/profile")]
        ProfilePage {},
        #[route("/access/car")]
        AccessCarPage {},
        #[route("/access/visitor")]
        AccessVisitorPage {},
        #[route("/access/visitor/register")]
        AccessVisitorRegisterPage {},
        #[route("/system/role")]
        RoleManagementPage {},
        #[route("/:..segments")]
        DynamicWorkspacePage { segments: Vec<String> },
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use dioxus::router::components::HistoryProvider;
    use dioxus_history::{History, MemoryHistory};
    use spacetimedb_sdk::Timestamp;

    use super::*;

    #[component]
    fn MaintenanceRouteRenderRoot() -> Element {
        let mut state = crate::app::use_workspace_state();
        state.auth_state.set(AuthState::SignedIn);
        state.current_user.set(Some(
            crate::spacetime_bindings::center_user_type::CenterUser {
                id: 1,
                username: "admin".into(),
                real_name: "系统管理员".into(),
                customer_type: None,
                status: 1,
                token_version: 1,
                phone: None,
                home_path: None,
                membership_trial_start_at: None,
                created_at: Timestamp::UNIX_EPOCH,
                updated_at: None,
            },
        ));
        state
            .roles
            .set(vec![crate::spacetime_bindings::role_type::Role {
                role_id: 1,
                customer_id: "public".into(),
                name: "Super".into(),
                remark: None,
                status: 1,
                rates: None,
                parent_id: None,
                reimbursement_auth: None,
                organization_id: None,
                scope: "system".into(),
                created_at: Timestamp::UNIX_EPOCH,
                updated_at: None,
            }]);
        use_context_provider(|| state);

        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/maintenance/transformer")) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    #[component]
    fn MaintenanceRouteTransitionRoot() -> Element {
        let mut state = crate::app::use_workspace_state();
        state.auth_state.set(AuthState::SignedIn);
        state.current_user.set(Some(
            crate::spacetime_bindings::center_user_type::CenterUser {
                id: 1,
                username: "admin".into(),
                real_name: "系统管理员".into(),
                customer_type: None,
                status: 1,
                token_version: 1,
                phone: None,
                home_path: None,
                membership_trial_start_at: None,
                created_at: Timestamp::UNIX_EPOCH,
                updated_at: None,
            },
        ));
        state
            .roles
            .set(vec![crate::spacetime_bindings::role_type::Role {
                role_id: 1,
                customer_id: "public".into(),
                name: "Super".into(),
                remark: None,
                status: 1,
                rates: None,
                parent_id: None,
                reimbursement_auth: None,
                organization_id: None,
                scope: "system".into(),
                created_at: Timestamp::UNIX_EPOCH,
                updated_at: None,
            }]);
        use_context_provider(|| state);

        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    #[test]
    fn 智能水电表管理的电表和水表路由均可解析() {
        assert!(matches!(
            "/smart-meter/meter".parse::<Route>(),
            Ok(Route::SmartElectricMeterPage {})
        ));
        assert!(matches!(
            "/smart-meter/water".parse::<Route>(),
            Ok(Route::SmartWaterMeterPage {})
        ));
    }

    #[test]
    fn 设备管理的三条路由均可解析() {
        assert!(matches!(
            "/device/overview".parse::<Route>(),
            Ok(Route::DeviceOverviewPage {})
        ));
        assert!(matches!(
            "/device/camera".parse::<Route>(),
            Ok(Route::DeviceCameraPage {})
        ));
        assert!(matches!(
            "/device/access".parse::<Route>(),
            Ok(Route::DeviceAccessPage {})
        ));
        assert!(matches!(
            "/device/gateway".parse::<Route>(),
            Ok(Route::DeviceGatewayPage {})
        ));
    }

    #[test]
    fn 账期结转路由可以解析() {
        assert!(matches!(
            "/bill/carryover".parse::<Route>(),
            Ok(Route::BillCarryoverPage {})
        ));
    }

    #[test]
    fn 账单管理路由可以解析() {
        assert!(matches!(
            "/bill".parse::<Route>(),
            Ok(Route::BillManagementPage {})
        ));
    }

    #[test]
    fn 财务总览路由可以解析() {
        assert!(matches!(
            "/finance/overview".parse::<Route>(),
            Ok(Route::FinanceOverviewPage {})
        ));
    }

    #[test]
    fn 数据地图路由可以解析() {
        assert!(matches!(
            "/data-map".parse::<Route>(),
            Ok(Route::DataMapPage {})
        ));
    }

    #[test]
    fn 合同管理路由可以解析() {
        assert!(matches!(
            "/rental/tenant".parse::<Route>(),
            Ok(Route::ContractManagementPage {})
        ));
    }

    /// 合同表单的三条路由都挂在 `/rental/tenant` 下面，容易被列表页那条
    /// 抢先匹配掉。分别解析一次，确认段数不同的路由没有互相遮蔽。
    #[test]
    fn 合同表单的三条路由都可以解析() {
        assert!(matches!(
            "/rental/tenant/new".parse::<Route>(),
            Ok(Route::ContractCreatePage {})
        ));
        assert!(matches!(
            "/rental/tenant/edit/12".parse::<Route>(),
            Ok(Route::ContractEditPage { id: 12 })
        ));
        assert!(matches!(
            "/rental/tenant/detail/34".parse::<Route>(),
            Ok(Route::ContractDetailPage { id: 34 })
        ));
    }

    /// 打卡点设置挂在 `/hrm/attendance/` 下面，和打卡页同级，容易被写错成子路径。
    #[test]
    fn 打卡点设置路由可以解析() {
        assert!(matches!(
            "/hrm/attendance/location".parse::<Route>(),
            Ok(Route::HrmAttendanceLocationPage {})
        ));
    }

    #[test]
    fn 园区管理路由可以解析() {
        assert!(matches!(
            "/rental/manage".parse::<Route>(),
            Ok(Route::RentalManagementPage {})
        ));
    }

    #[test]
    fn 园区档案路由可以解析() {
        assert!(matches!(
            "/rental/detail/12".parse::<Route>(),
            Ok(Route::RentalParkDetailPage { id: 12 })
        ));
    }

    #[test]
    fn 报销管理和审核路由均可解析() {
        assert!(matches!(
            "/reimbursement/application".parse::<Route>(),
            Ok(Route::ReimbursementApplicationPage {})
        ));
        assert!(matches!(
            "/reimbursement/audit".parse::<Route>(),
            Ok(Route::ReimbursementAuditPage {})
        ));
    }

    #[test]
    fn 人事管理六个业务路由均可解析() {
        for path in [
            "/hrm/overview",
            "/hrm/information",
            "/hrm/attendance/punch",
            "/hrm/attendance/stats",
            "/hrm/trajectory",
            "/hrm/leaveapplication",
        ] {
            assert!(path.parse::<Route>().is_ok(), "无法解析 {path}");
        }
    }

    #[test]
    fn 三个门禁管理路由均可解析() {
        for path in ["/access/car", "/access/visitor", "/access/visitor/register"] {
            assert!(path.parse::<Route>().is_ok(), "无法解析 {path}");
        }
    }

    #[test]
    fn 扫码巡检路由可以带出设备编号() {
        assert!(matches!(
            "/maintenance/inspect/transformer/7".parse::<Route>(),
            Ok(Route::MaintenanceInspectTransformerPage { id: 7 })
        ));
        assert!(matches!(
            "/maintenance/inspect/elevator/9".parse::<Route>(),
            Ok(Route::MaintenanceInspectElevatorPage { id: 9 })
        ));
        assert!(matches!(
            "/maintenance/inspect/firefighting/11".parse::<Route>(),
            Ok(Route::MaintenanceInspectFirefightingPage { id: 11 })
        ));
    }

    #[test]
    fn 维护页可以从查询参数带出厂房筛选() {
        let route = "/maintenance/transformer?factory=1号厂房"
            .parse::<Route>()
            .expect("带查询参数的维护路由应当可解析");
        let Route::MaintenanceTransformerPage { factory } = &route else {
            panic!("解析到了别的路由：{route:?}");
        };
        assert_eq!(factory, "1号厂房");
        // 权限判定与侧边栏高亮都比对纯路径，查询串必须被剥掉。
        assert_eq!(route_path(&route), "/maintenance/transformer");
    }

    #[test]
    fn 个人中心路由可以解析() {
        assert!(matches!(
            "/profile".parse::<Route>(),
            Ok(Route::ProfilePage {})
        ));
    }

    #[test]
    fn 四个维护管理路由均可解析() {
        assert!(matches!(
            "/maintenance/firefighting".parse::<Route>(),
            Ok(Route::MaintenanceFirefightingPage { .. })
        ));
        assert!(matches!(
            "/maintenance/transformer".parse::<Route>(),
            Ok(Route::MaintenanceTransformerPage { .. })
        ));
        assert!(matches!(
            "/maintenance/elevator".parse::<Route>(),
            Ok(Route::MaintenanceElevatorPage { .. })
        ));
        assert!(matches!(
            "/maintenance/repair-order".parse::<Route>(),
            Ok(Route::MaintenanceRepairOrderPage {})
        ));
    }

    /// 只设置登录态，用于验证 WorkspaceGate 的三个分支各自渲染什么。
    #[component]
    fn AuthGateRenderRoot(auth_state: AuthState) -> Element {
        let mut state = crate::app::use_workspace_state();
        state.auth_state.set(auth_state);
        use_context_provider(|| state);

        rsx! {
            HistoryProvider {
                history: move |_| Rc::new(MemoryHistory::with_initial_path("/")) as Rc<dyn History>,
                Router::<Route> {}
            }
        }
    }

    #[test]
    fn 恢复登录态期间不渲染登录表单() {
        let html = dioxus_ssr::render_element(rsx! {
            AuthGateRenderRoot { auth_state: AuthState::Checking }
        });
        assert!(html.contains("正在恢复登录状态"), "缺少恢复中的占位屏");
        assert!(
            !html.contains("密码登录"),
            "会话有效时登录表单会先闪一下，观感上等同于要求重新登录"
        );
    }

    #[test]
    fn 确认已登出后才渲染登录表单() {
        let html = dioxus_ssr::render_element(rsx! {
            AuthGateRenderRoot { auth_state: AuthState::SignedOut }
        });
        assert!(html.contains("密码登录"), "已登出应当给出登录入口");
        assert!(!html.contains("正在恢复登录状态"), "不应停留在占位屏");
    }

    #[test]
    fn 变压器地址通过工作区_outlet_渲染真实页面() {
        let html = dioxus_ssr::render_element(rsx! { MaintenanceRouteRenderRoot {} });
        assert!(html.contains("变压器管理"), "工作区没有渲染变压器页面");
        assert!(html.contains("变压器管理台账"), "Outlet 仍停留在其他页面");
        assert!(
            !html.contains("园区经营总览"),
            "Outlet 错误地保留了首页内容"
        );
    }

    #[test]
    fn 客户端从首页切换后_outlet_替换为变压器页面() {
        TEST_NAVIGATION_TARGET.with(|target| {
            *target.borrow_mut() = Some("/maintenance/transformer".into());
        });
        let mut dom = VirtualDom::new(MaintenanceRouteTransitionRoot);
        dom.rebuild_in_place();
        assert!(dioxus_ssr::render(&dom).contains("园区经营总览"));
        dom.render_immediate(&mut dioxus::core::NoOpMutations);

        let html = dioxus_ssr::render(&dom);
        assert!(
            html.contains("变压器管理台账"),
            "路由切换后 Outlet 没有更新"
        );
        assert!(!html.contains("园区经营总览"), "路由切换后仍保留首页内容");
    }

    #[test]
    fn 原系统角色管理路由进入真实页面() {
        assert!(matches!(
            "/system/role".parse::<Route>(),
            Ok(Route::RoleManagementPage {})
        ));
    }
}
