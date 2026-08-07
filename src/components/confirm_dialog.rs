//! 全站共用的删除/危险操作确认弹窗。
//!
//! 组件库的 `AlertDialog` 在本应用里点开没有反应，而同源的 `Dialog` 工作正常，
//! 所以这里基于 `Dialog` 实现，用 `role="alertdialog"` 保留告警语义。
//! 按钮也换成本应用的 `Button`，与其余弹窗保持同一套外观——`AlertDialog`
//! 自带的取消/确认按钮是另一套 1rem 字号的样式，和全站对不上。

use dioxus::prelude::*;

use crate::components::{
    button::{Button, ButtonVariant},
    dialog::{Dialog, DialogDescription, DialogTitle},
};

/// 受控确认弹窗：由调用方决定何时挂载，这里恒为打开。
#[component]
pub fn ConfirmDialog(
    title: String,
    description: String,
    /// 确认按钮的文案，例如「确认删除」。
    confirm_label: String,
    /// 请求进行中：按钮禁用，且不允许 Esc 或点击遮罩关闭。
    #[props(default)]
    busy: bool,
    #[props(default)] error: Option<String>,
    /// 这次操作注定会被服务端拒绝：确认按钮禁用，只留取消。
    ///
    /// 用于前端已经能判定条件不满足的场合——显示一个点了必然报错的按钮，
    /// 比直接说明原因更让人困惑。
    #[props(default)]
    confirm_disabled: bool,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog {
            open: Some(true),
            role: "alertdialog",
            on_open_change: move |open: bool| {
                if !open && !busy {
                    on_cancel.call(());
                }
            },
            DialogTitle { "{title}" }
            DialogDescription { "{description}" }

            if let Some(message) = error.as_ref() {
                p { class: "form-error", role: "alert", "{message}" }
            }

            div { class: "form-actions",
                Button {
                    variant: ButtonVariant::Outline,
                    r#type: "button",
                    disabled: busy,
                    onclick: move |_| on_cancel.call(()),
                    "取消"
                }
                Button {
                    variant: ButtonVariant::Destructive,
                    r#type: "button",
                    disabled: busy || confirm_disabled,
                    onclick: move |_| on_confirm.call(()),
                    if busy {
                        "处理中…"
                    } else {
                        "{confirm_label}"
                    }
                }
            }
        }
    }
}
