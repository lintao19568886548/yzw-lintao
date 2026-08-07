use dioxus::prelude::*;
use dioxus_icons::lucide::ChevronDown;
use dioxus_primitives::collapsible::{
    self, CollapsibleContentProps, CollapsibleProps, CollapsibleTriggerProps,
};
use dioxus_primitives::dioxus_attributes::attributes;
use dioxus_primitives::merge_attributes;

#[css_module("/src/components/collapsible/style.css")]
struct Styles;

#[component]
pub fn Collapsible(props: CollapsibleProps) -> Element {
    rsx! {
        collapsible::Collapsible {
            keep_mounted: props.keep_mounted,
            default_open: props.default_open,
            disabled: props.disabled,
            open: props.open,
            on_open_change: props.on_open_change,
            as: props.r#as,
            attributes: props.attributes,
            {props.children}
        }
    }
}

#[component]
pub fn CollapsibleTrigger(props: CollapsibleTriggerProps) -> Element {
    let base = attributes!(button {
        class: Styles::dx_collapsible_trigger,
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    let show_icon = props.r#as.is_none();

    rsx! {
        collapsible::CollapsibleTrigger { as: props.r#as, attributes: merged,
            {props.children}
            if show_icon {
                // 上游用的是 ChevronsUpDown（⇅），那是排序/换位的语义，
                // 会让人以为这一行能拖动。折叠面板要的是单向箭头，
                // 展开态由 CSS 依 data-state 旋转 180 度。
                ChevronDown {
                    size: "1rem",
                    stroke: "var(--secondary-color-3)",
                    class: "dx-collapsible-chevron",
                }
            }
        }
    }
}

#[component]
pub fn CollapsibleContent(props: CollapsibleContentProps) -> Element {
    // CollapsibleContentProps 没有独立的 class 字段，class 会落进 attributes。
    // 上游同时写 `class:` 和 `attributes:`，两者指向同一个字段，调用方传进来的
    // 类名会被整条丢掉——外面挂的 .collapse-body 之类根本不会出现在 DOM 上。
    // 按 Dialog 的写法先合并再传。
    let base = attributes!(div {
        class: Styles::dx_collapsible_content,
    });
    let merged = merge_attributes(vec![base, props.attributes]);

    rsx! {
        collapsible::CollapsibleContent { id: props.id, attributes: merged, {props.children} }
    }
}

#[component]
pub fn CollapsibleItem(
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        div {
            border: "1px solid var(--primary-color-6)",
            border_radius: "0.5rem",
            padding: "1rem",
            ..attributes,
            {children}
        }
    }
}

#[component]
pub fn CollapsibleList(
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        div {
            display: "flex",
            flex_direction: "column",
            gap: "0.5rem",
            max_width: "20rem",
            color: "var(--secondary-color-3)",
            ..attributes,
            {children}
        }
    }
}
