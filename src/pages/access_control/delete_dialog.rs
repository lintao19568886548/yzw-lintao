//! 门禁记录删除确认框。

use dioxus::prelude::*;

use crate::{
    components::ConfirmDialog,
    services::{delete_access_car_record, delete_access_visitor_record},
};

#[derive(Clone, PartialEq)]
pub(super) enum AccessDeleteTarget {
    Car { id: u64, name: String },
    Visitor { id: u64, name: String },
}

#[component]
pub(super) fn AccessDeleteDialog(
    target: AccessDeleteTarget,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut completed = use_signal(|| false);

    use_effect(move || {
        if completed() {
            on_deleted.call(());
        }
    });

    let (kind, name) = match &target {
        AccessDeleteTarget::Car { name, .. } => ("车辆", name.clone()),
        AccessDeleteTarget::Visitor { name, .. } => ("访客", name.clone()),
    };

    rsx! {
        ConfirmDialog {
            title: format!("删除{kind}记录"),
            description: format!("确定删除“{name}”的这条通行记录吗？此操作会真正移除记录。"),
            confirm_label: "确认删除",
            busy: loading(),
            error: error(),
            on_cancel: move |_| on_close.call(()),
            on_confirm: move |_| {
                if loading() {
                    return;
                }
                loading.set(true);
                error.set(None);
                let next = target.clone();
                spawn(async move {
                    let result = match next {
                        AccessDeleteTarget::Car { id, .. } => delete_access_car_record(id).await,
                        AccessDeleteTarget::Visitor { id, .. } => {
                            delete_access_visitor_record(id).await
                        }
                    };
                    match result {
                        Ok(()) => completed.set(true),
                        Err(message) => {
                            loading.set(false);
                            error.set(Some(message));
                        }
                    }
                });
            },
        }
    }
}
