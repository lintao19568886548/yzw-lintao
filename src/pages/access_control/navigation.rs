//! 门禁管理二级导航。

use dioxus::prelude::*;

use crate::{
    components::button::{Button, ButtonVariant},
    permissions::can_access_route,
    state::WorkspaceState,
};

#[component]
pub(super) fn AccessNavigation(active: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();

    // 三项各自是独立路由，用按钮外观的链接而不是 Tabs——Tabs 是同页切换面板，
    // 浏览器前进后退会和高亮对不上。
    rsx! {
        nav { class: "row", aria_label: "门禁管理模块",
            if can_access_route(&roles, &menus, "/access/car") {
                Link { to: crate::router::Route::AccessCarPage {},
                    Button {
                        variant: if active == "car" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "车辆出入管理"
                    }
                }
            }
            if can_access_route(&roles, &menus, "/access/visitor") {
                Link { to: crate::router::Route::AccessVisitorPage {},
                    Button {
                        variant: if active == "visitor" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "访客管理"
                    }
                }
            }
            if can_access_route(&roles, &menus, "/access/visitor/register") {
                Link { to: crate::router::Route::AccessVisitorRegisterPage {},
                    Button {
                        variant: if active == "register" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "访客登记"
                    }
                }
            }
        }
    }
}
