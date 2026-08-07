//! 模块总览页共用的"数据流向"展示：一条链路里的单个阶段。
//!
//! 从 `pages::rental_management::flow` 提出来——`pages::hr::flow` 出现
//! 第二个调用点时，同一份标记和 CSS（`.data-flow*`，见
//! `assets/styles/20-app.css`）就不该在两个模块里各写一份。

use dioxus::prelude::*;

/// 链路里的单个阶段：有权限就整块可点击跳转，没权限就只展示只读数字。
///
/// 填充条宽度按 `count / max_count` 归一化，而不是按原始数值——同一条
/// 页面里往往有好几条链路共用同一个 `max_count`，这样同一个数字在不同
/// 链路里显示的条宽是一致的，能一眼看出是同一份数据。
#[component]
pub fn DataFlowStage(
    label: &'static str,
    caption: String,
    count: usize,
    max_count: usize,
    to: Option<crate::router::Route>,
    #[props(default)] children: Element,
) -> Element {
    let fill_percent = (count as f64 / max_count as f64 * 100.0).clamp(0.0, 100.0);
    let body = rsx! {
        span { class: "data-flow-stage-label", "{label}" }
        span { class: "data-flow-stage-count", "{count}" }
        div { class: "data-flow-bar",
            span { style: "width: {fill_percent}%" }
        }
        span { class: "data-flow-stage-label", "{caption}" }
        {children}
    };
    match to {
        Some(route) => rsx! {
            Link { to: route, class: "data-flow-stage", {body} }
        },
        None => rsx! {
            div { class: "data-flow-stage", {body} }
        },
    }
}
