//! 工作空间顶部状态栏。

use dioxus::prelude::*;

use crate::{
    components::{sidebar::SidebarTrigger, ConnectivityStatus, Icon},
    state::WorkspaceState,
};

#[component]
pub fn Topbar() -> Element {
    let state = use_context::<WorkspaceState>();
    let phase = (state.phase)();
    let user = state.current_user.read().clone();
    // 头像挂在运营方业务用户上，顶栏的姓名与角色则来自中心账号。
    let avatar_url = (state.business_user)().and_then(|user| user.avatar_url);
    let role_label = state
        .roles
        .read()
        .first()
        .map(|role| role.name.clone())
        .unwrap_or_else(|| "普通成员".into());

    rsx! {
        header { class: "topbar",
            div { class: "topbar-leading",
                // 展开/收起与移动端抽屉都由组件库的触发器统一处理。
                SidebarTrigger {}
                div { class: "workspace-heading",
                    span { class: "eyebrow", "OPERATIONS DESK" }
                    strong { "园区运营工作台" }
                }
            }

            div { class: "topbar-actions",
                label { class: "command-search",
                    Icon { name: "search" }
                    input {
                        aria_label: "搜索功能",
                        placeholder: "搜索功能、园区或客户",
                    }
                    kbd { "⌘ K" }
                }
                ConnectivityStatus { phase, compact: true }
                button { class: "icon-button notification-button", aria_label: "通知",
                    Icon { name: "bell" }
                    span { class: "notification-dot" }
                }
                if let Some(user) = user {
                    // 做成按钮而不是纯展示：窄屏下姓名与角色让位给空间，
                    // 头像成了用户区唯一可见的元素，它必须能点。
                    button {
                        class: "user-chip",
                        r#type: "button",
                        aria_label: "打开个人中心",
                        onclick: move |_| {
                            navigator().push(crate::router::Route::ProfilePage {});
                        },
                        if let Some(url) = avatar_url.clone() {
                            img { class: "user-avatar", src: "{url}", alt: "" }
                        } else {
                            span { class: "user-avatar", "{user.real_name.chars().next().unwrap_or('云')}" }
                        }
                        span { class: "user-chip-copy",
                            strong { "{user.real_name}" }
                            small { "{role_label}" }
                        }
                    }
                }
            }
        }
    }
}
