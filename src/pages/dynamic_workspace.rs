//! 动态菜单对应的业务页面容器。

use dioxus::prelude::*;

use crate::{
    components::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        card::{Card, CardContent},
    },
    state::WorkspaceState,
};

#[component]
pub fn DynamicWorkspacePage(segments: Vec<String>) -> Element {
    let state = use_context::<WorkspaceState>();
    let path = format!("/{}", segments.join("/"));
    let menu = state
        .menus
        .read()
        .iter()
        .find(|menu| menu.path == path || menu.active_path.as_deref() == Some(path.as_str()))
        .cloned();

    let Some(menu) = menu else {
        return rsx! {
            main { class: "page",
                Card {
                    CardContent {
                        div { class: "stack",
                            h1 { "当前身份无权访问此页面" }
                            p { class: "page-subtitle",
                                "路径 {path} 不在实时菜单权限中，或者菜单已经被管理员停用。"
                            }
                            div { class: "card-cta",
                                Link { to: crate::router::Route::DashboardPage {},
                                    Button { variant: ButtonVariant::Outline, "返回工作台" }
                                }
                            }
                        }
                    }
                }
            }
        };
    };

    let component_name = menu.component.unwrap_or_else(|| "动态业务组件".into());

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    div { class: "row",
                        Link { class: "hint", to: crate::router::Route::DashboardPage {}, "工作台" }
                        span { class: "hint", "/" }
                        span { class: "hint", "{menu.name}" }
                    }
                    h1 { "{menu.name}" }
                    p { class: "page-subtitle",
                        "该路由由 SpacetimeDB my_menus 视图动态提供，尚未接入具体业务页面。"
                    }
                }
                div { class: "page-actions",
                    Badge { variant: BadgeVariant::Secondary, "权限已验证" }
                }
            }

            Card {
                CardContent {
                    div { class: "stack",
                        h2 { "页面容器已经就绪" }
                        dl { class: "facts",
                            div {
                                dt { "菜单路径" }
                                dd { class: "is-mono", "{menu.path}" }
                            }
                            div {
                                dt { "原组件标识" }
                                dd { class: "is-mono", "{component_name}" }
                            }
                            div {
                                dt { "订阅查询" }
                                dd { class: "is-mono", "my_* 视图" }
                            }
                            div {
                                dt { "写入操作" }
                                dd { class: "is-mono", "Reducer" }
                            }
                        }
                    }
                }
            }
        }
    }
}
