use dioxus::prelude::*;
use dioxus_primitives::alert_dialog::{
    self, AlertDialogActionProps, AlertDialogActionsProps, AlertDialogCancelProps,
    AlertDialogDescriptionProps, AlertDialogRootProps, AlertDialogTitleProps,
};
use dioxus_primitives::{dioxus_attributes::attributes, merge_attributes};

#[css_module("/src/components/alert_dialog/style.css")]
struct Styles;

#[component]
pub fn AlertDialog(props: AlertDialogRootProps) -> Element {
    // 同时写 `class:` 和 `attributes:` 会指向同一个字段，其中一个被整条丢掉。
    // 先合并再传，与 Dialog 的写法保持一致。
    let base = attributes!(div {
        class: Styles::dx_alert_dialog_backdrop,
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        alert_dialog::AlertDialogRoot {
            id: props.id,
            default_open: props.default_open,
            open: props.open,
            on_open_change: props.on_open_change,
            attributes: merged,
            alert_dialog::AlertDialogContent {
                class: Styles::dx_alert_dialog.to_string(),
                {props.children}
            }
        }
    }
}

#[component]
pub fn AlertDialogTitle(props: AlertDialogTitleProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogTitle {
            class: Styles::dx_alert_dialog_title,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogDescription(props: AlertDialogDescriptionProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogDescription {
            class: Styles::dx_alert_dialog_description,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogActions(props: AlertDialogActionsProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogActions { class: Styles::dx_alert_dialog_actions, attributes: props.attributes, {props.children} }
    }
}

#[component]
pub fn AlertDialogCancel(props: AlertDialogCancelProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogCancel {
            on_click: props.on_click,
            class: Styles::dx_alert_dialog_cancel,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn AlertDialogAction(props: AlertDialogActionProps) -> Element {
    rsx! {
        alert_dialog::AlertDialogAction {
            class: Styles::dx_alert_dialog_action,
            on_click: props.on_click,
            attributes: props.attributes,
            {props.children}
        }
    }
}
