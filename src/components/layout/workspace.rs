//! 桌面和移动端共用的管理后台外壳。

use dioxus::prelude::*;

use crate::components::sidebar::{use_sidebar, SidebarInset, SidebarProvider};
use crate::state::{ModuleLoadState, WorkspaceState};

use super::{sidebar::AppSidebar, topbar::Topbar};

#[component]
pub fn WorkspaceLayout() -> Element {
    let state = use_context::<WorkspaceState>();
    let module_notice = (state.active_scope)().and_then(|scope| {
        state
            .module_states
            .read()
            .get(&scope)
            .and_then(|status| match status {
                ModuleLoadState::Loading => Some((false, format!("正在加载{}数据", scope.label()))),
                ModuleLoadState::Error(message) => {
                    Some((true, format!("{}加载失败：{message}", scope.label())))
                }
                ModuleLoadState::Idle | ModuleLoadState::Ready => None,
            })
    });

    rsx! {
        SidebarProvider {
            AppSidebar {}
            SidebarInset {
                // 左缘上滑手势打开移动端抽屉，保留原有的单手操作体验。
                EdgeSwipeZone {}
                Topbar {}
                if let Some((is_error, message)) = module_notice {
                    div {
                        class: if is_error { "module-load-status is-error" } else { "module-load-status is-loading" },
                        role: "status",
                        aria_live: "polite",
                        "{message}"
                    }
                }
                main {
                    class: "workspace-content",
                    Outlet::<crate::router::Route> {}
                }
            }
        }
    }
}

/// 屏幕左缘的滑动感应区：向右滑打开移动端导航抽屉。
///
/// 组件库的 Sidebar 只提供按钮触发，这里补回原有的手势入口。
#[component]
fn EdgeSwipeZone() -> Element {
    let ctx = use_sidebar();
    let mut edge_touch_start = use_signal(|| None::<(f64, f64)>);

    rsx! {
        div {
            class: "sidebar-edge-swipe-zone",
            aria_hidden: "true",
            ontouchstart: move |event| {
                let touches = event.touches();
                if let Some(touch) = touches.first() {
                    let point = touch.client_coordinates();
                    edge_touch_start.set(Some((point.x, point.y)));
                }
            },
            ontouchend: move |event| {
                let Some((start_x, start_y)) = edge_touch_start() else {
                    return;
                };
                edge_touch_start.set(None);

                let changed_touches = event.touches_changed();
                let Some(touch) = changed_touches.first() else {
                    return;
                };
                let point = touch.client_coordinates();
                let horizontal_distance = point.x - start_x;
                let vertical_distance = (point.y - start_y).abs();

                if start_x <= 28.0
                    && horizontal_distance >= 56.0
                    && horizontal_distance > vertical_distance * 1.25
                {
                    ctx.set_open_mobile(true);
                }
            },
            ontouchcancel: move |_| edge_touch_start.set(None),
        }
    }
}
