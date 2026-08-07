//! Dioxus 应用入口和身份门禁。

use dioxus::prelude::*;

use crate::{
    router::Route,
    services::{connect_workspace, use_auto_reconnect, ConnectionConfig},
    state::{AuthState, ConnectionPhase, WorkspaceState},
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
/// 小尺寸变体：满幅印面、无云纹，16px 下仍认得出（见文件内注释）。
const FAVICON_SVG: Asset = asset!("/assets/logo-mark.svg");
const APPLE_TOUCH_ICON: Asset = asset!("/assets/apple-touch-icon.png");
/// dioxus-components 的主题变量，必须先于组件样式加载。
const DX_THEME_CSS: Asset = asset!("/assets/dx-components-theme.css");
const BASE_CSS: Asset = asset!("/assets/styles/00-base.css");
const LAYOUT_CSS: Asset = asset!("/assets/styles/10-layout.css");
const APP_CSS: Asset = asset!("/assets/styles/20-app.css");

#[allow(non_snake_case)]
pub fn App() -> Element {
    let state = use_workspace_state();
    use_context_provider(|| state);

    use_effect(move || {
        let mut callback_state = state;
        spawn(async move {
            if let Err(error) = connect_workspace(ConnectionConfig::default(), callback_state).await
            {
                callback_state.phase.set(ConnectionPhase::Failed);
                callback_state.error_message.set(Some(error));
            }
        });
    });

    // 首次连接之后的自愈交给它：断线按退避重连，网络恢复时立刻重试。
    use_auto_reconnect(state);

    rsx! {
        // 微信内置浏览器默认会按桌面宽度排版，必须显式声明移动端视口。
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover"
        }
        // 现代浏览器优先用矢量图，缩放到任何尺寸都清晰；ico 留给不支持
        // SVG 图标的旧版本，PNG 供 iOS 与安卓添加到主屏时使用。
        document::Link { rel: "icon", r#type: "image/svg+xml", href: FAVICON_SVG }
        document::Link { rel: "alternate icon", href: FAVICON }
        document::Link { rel: "apple-touch-icon", href: APPLE_TOUCH_ICON }
        document::Stylesheet { href: DX_THEME_CSS }
        document::Stylesheet { href: BASE_CSS }
        document::Stylesheet { href: LAYOUT_CSS }
        document::Stylesheet { href: APP_CSS }
        document::Title { "云园慧控 · 园区运营工作台" }

        Router::<Route> {}
    }
}

/// 构造工作区的全部响应式状态。
///
/// 独立成自定义 Hook 后，业务页面可以在渲染测试中复用与真实应用完全相同的上下文，
/// 避免测试只验证路由字符串、却遗漏页面首次渲染异常。
pub(crate) fn use_workspace_state() -> WorkspaceState {
    WorkspaceState {
        // SSR 阶段客户端 WASM 尚未开始执行，但应用启动后一定会立即建立实时连接。
        // 初始状态直接标记为 Connecting，避免水合前错误显示“等待连接”。
        phase: use_signal_sync(|| ConnectionPhase::Connecting),
        auth_state: use_signal_sync(|| AuthState::Checking),
        active_scope: use_signal_sync(|| None),
        module_states: use_signal_sync(Default::default),
        current_user: use_signal_sync(|| None),
        business_user: use_signal_sync(|| None),
        menus: use_signal_sync(Vec::new),
        roles: use_signal_sync(Vec::new),
        permission_codes: use_signal_sync(Vec::new),
        permission_roles: use_signal_sync(Vec::new),
        permission_menus: use_signal_sync(Vec::new),
        permission_parks: use_signal_sync(Vec::new),
        permission_role_menus: use_signal_sync(Vec::new),
        permission_role_parks: use_signal_sync(Vec::new),
        dashboard_overview: use_signal_sync(|| None),
        rental_tenants: use_signal_sync(Vec::new),
        rental_tenant_floors: use_signal_sync(Vec::new),
        tenant_profiles: use_signal_sync(Vec::new),
        parks: use_signal_sync(Vec::new),
        park_image_previews: use_signal_sync(Vec::new),
        factories: use_signal_sync(Vec::new),
        factory_floors: use_signal_sync(Vec::new),
        factory_floor_image_previews: use_signal_sync(Vec::new),
        dormitories: use_signal_sync(Vec::new),
        dormitory_floors: use_signal_sync(Vec::new),
        rental_tenant_dormitory_floors: use_signal_sync(Vec::new),
        dormitory_image_previews: use_signal_sync(Vec::new),
        utility_meters: use_signal_sync(Vec::new),
        device_assets: use_signal_sync(Vec::new),
        device_asset_image_previews: use_signal_sync(Vec::new),
        edge_gateways: use_signal_sync(Vec::new),
        rental_tenant_meters: use_signal_sync(Vec::new),
        rental_tenant_fees: use_signal_sync(Vec::new),
        firefighting_assets: use_signal_sync(Vec::new),
        firefighting_inspections: use_signal_sync(Vec::new),
        firefighting_asset_image_previews: use_signal_sync(Vec::new),
        firefighting_inspection_image_previews: use_signal_sync(Vec::new),
        transformer_assets: use_signal_sync(Vec::new),
        transformer_inspections: use_signal_sync(Vec::new),
        transformer_asset_image_previews: use_signal_sync(Vec::new),
        transformer_inspection_image_previews: use_signal_sync(Vec::new),
        elevator_assets: use_signal_sync(Vec::new),
        elevator_inspections: use_signal_sync(Vec::new),
        elevator_asset_image_previews: use_signal_sync(Vec::new),
        elevator_inspection_image_previews: use_signal_sync(Vec::new),
        repair_orders: use_signal_sync(Vec::new),
        access_cars: use_signal_sync(Vec::new),
        access_visitors: use_signal_sync(Vec::new),
        salaries: use_signal_sync(Vec::new),
        salary_image_previews: use_signal_sync(Vec::new),
        reimbursements: use_signal_sync(Vec::new),
        reimbursement_image_previews: use_signal_sync(Vec::new),
        employees: use_signal_sync(Vec::new),
        employee_user_options: use_signal_sync(Vec::new),
        attendances: use_signal_sync(Vec::new),
        localizations: use_signal_sync(Vec::new),
        leave_applications: use_signal_sync(Vec::new),
        attendance_locations: use_signal_sync(Vec::new),
        attendance_abnormal_logs: use_signal_sync(Vec::new),
        tenant_image_previews: use_signal_sync(Vec::new),
        amount_bills: use_signal_sync(Vec::new),
        bill_collection_confirmations: use_signal_sync(Vec::new),
        carryover_batches: use_signal_sync(Vec::new),
        carryover_items: use_signal_sync(Vec::new),
        finances: use_signal_sync(Vec::new),
        finance_images: use_signal_sync(Vec::new),
        ele_bills: use_signal_sync(Vec::new),
        water_bills: use_signal_sync(Vec::new),
        identity: use_signal_sync(|| None),
        authenticating: use_signal_sync(|| false),
        error_message: use_signal_sync(|| None),
        last_synced_label: use_signal_sync(|| "尚未同步".to_string()),
    }
}
