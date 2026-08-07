//! 智能水电表管理页面左侧园区、楼栋、楼层与设备树。

use std::collections::{BTreeMap, BTreeSet};

use dioxus::prelude::*;

use crate::services::SmartMeterDevice;

type FloorDevices = BTreeMap<String, Vec<SmartMeterDevice>>;
type BuildingFloors = BTreeMap<String, FloorDevices>;
type ParkBuildings = BTreeMap<String, BuildingFloors>;

#[component]
pub(super) fn MeterDeviceTree(
    devices: Vec<SmartMeterDevice>,
    search: String,
    selected: String,
    on_select: EventHandler<String>,
) -> Element {
    // 默认展开全部节点，与原 Ant Design Tree 首次加载后的行为一致。
    // 这里只记录已收起的节点，后续设备目录刷新时新节点仍会自然展开。
    let mut collapsed_nodes = use_signal(BTreeSet::<String>::new);
    let keyword = search.trim().to_lowercase();
    let filtered = devices
        .into_iter()
        .filter(|device| {
            keyword.is_empty()
                || device.park_name.to_lowercase().contains(&keyword)
                || device.building_name.to_lowercase().contains(&keyword)
                || device.floor_name.to_lowercase().contains(&keyword)
                || device.room_name.to_lowercase().contains(&keyword)
                || device.factory_no.to_lowercase().contains(&keyword)
                || device.device_id.to_lowercase().contains(&keyword)
        })
        .collect::<Vec<_>>();
    let mut parks = ParkBuildings::new();
    for device in filtered.iter().cloned() {
        let park_name = display_or(&device.park_name, "未分配园区");
        let building_name = display_or(&device.building_name, "设备档案");
        let floor_name = display_or(&device.floor_name, "设备列表");
        parks
            .entry(park_name)
            .or_default()
            .entry(building_name)
            .or_default()
            .entry(floor_name)
            .or_default()
            .push(device);
    }
    let collapsed_snapshot = collapsed_nodes();

    rsx! {
        div { class: "tree", role: "tree",
            button {
                class: if selected.is_empty() { "tree-label is-active" } else { "tree-label" },
                r#type: "button",
                onclick: move |_| on_select.call(String::new()),
                strong { "全部设备" }
                small { class: "hint", "{filtered.len()} 台" }
            }
            if parks.is_empty() {
                p { class: "hint", "没有匹配的建筑或设备" }
            } else {
                for (park_name , buildings) in parks {
                    {
                        let park_key = format!("park:{park_name}");
                        let park_key_for_click = park_key.clone();
                        let park_key_for_toggle = park_key.clone();
                        let park_expanded = !collapsed_snapshot.contains(&park_key) || !keyword.is_empty();
                        rsx! {
                            div { class: "tree-node", key: "park-{park_name}",
                                div {
                                    class: "tree-row",
                                    role: "treeitem",
                                    aria_expanded: "{park_expanded}",
                                    button {
                                        class: "tree-toggle",
                                        r#type: "button",
                                        aria_label: if park_expanded { "收起园区" } else { "展开园区" },
                                        onclick: move |_| {
                                            collapsed_nodes.with_mut(|nodes| toggle_collapsed(nodes, &park_key_for_toggle))
                                        },
                                        if park_expanded { "−" } else { "+" }
                                    }
                                    button {
                                        class: if selected == park_key { "tree-label is-active" } else { "tree-label" },
                                        r#type: "button",
                                        onclick: move |_| on_select.call(park_key_for_click.clone()),
                                        strong { "{park_name}" }
                                    }
                                }
                                if park_expanded {
                                    div { class: "tree-branch", role: "group",
                                        for (building_name , floors) in buildings {
                                            {
                                                let building_key = format!("building:{park_name}/{building_name}");
                                                let building_key_for_click = building_key.clone();
                                                let building_key_for_toggle = building_key.clone();
                                                let building_expanded = !collapsed_snapshot.contains(&building_key)
                                                    || !keyword.is_empty();
                                                rsx! {
                                                    div { class: "tree-node", key: "building-{park_name}-{building_name}",
                                                        div {
                                                            class: "tree-row",
                                                            role: "treeitem",
                                                            aria_expanded: "{building_expanded}",
                                                            button {
                                                                class: "tree-toggle",
                                                                r#type: "button",
                                                                aria_label: if building_expanded { "收起楼栋" } else { "展开楼栋" },
                                                                onclick: move |_| {
                                                                    collapsed_nodes
                                                                        .with_mut(|nodes| toggle_collapsed(nodes, &building_key_for_toggle))
                                                                },
                                                                if building_expanded { "−" } else { "+" }
                                                            }
                                                            button {
                                                                class: if selected == building_key { "tree-label is-active" } else { "tree-label" },
                                                                r#type: "button",
                                                                onclick: move |_| on_select.call(building_key_for_click.clone()),
                                                                "{building_name}"
                                                            }
                                                        }
                                                        if building_expanded {
                                                            div { class: "tree-branch", role: "group",
                                                                for (floor_name , floor_devices) in floors {
                                                                    {
                                                                        let floor_key = format!("floor:{park_name}/{building_name}/{floor_name}");
                                                                        let floor_key_for_click = floor_key.clone();
                                                                        let floor_key_for_toggle = floor_key.clone();
                                                                        let floor_expanded = !collapsed_snapshot.contains(&floor_key)
                                                                            || !keyword.is_empty();
                                                                        rsx! {
                                                                            div { class: "tree-node", key: "floor-{park_name}-{building_name}-{floor_name}",
                                                                                div {
                                                                                    class: "tree-row",
                                                                                    role: "treeitem",
                                                                                    aria_expanded: "{floor_expanded}",
                                                                                    button {
                                                                                        class: "tree-toggle",
                                                                                        r#type: "button",
                                                                                        aria_label: if floor_expanded { "收起楼层" } else { "展开楼层" },
                                                                                        onclick: move |_| {
                                                                                            collapsed_nodes.with_mut(|nodes| toggle_collapsed(nodes, &floor_key_for_toggle))
                                                                                        },
                                                                                        if floor_expanded { "−" } else { "+" }
                                                                                    }
                                                                                    button {
                                                                                        class: if selected == floor_key { "tree-label is-active" } else { "tree-label" },
                                                                                        r#type: "button",
                                                                                        onclick: move |_| on_select.call(floor_key_for_click.clone()),
                                                                                        "{floor_name}"
                                                                                    }
                                                                                }
                                                                                if floor_expanded {
                                                                                    div { class: "tree-branch", role: "group",
                                                                                        for device in floor_devices {
                                                                                            {
                                                                                                let device_key = format!("device:{}", device.factory_no);
                                                                                                let device_key_for_click = device_key.clone();
                                                                                                rsx! {
                                                                                                    button {
                                                                                                        key: "device-{device.factory_no}",
                                                                                                        class: if selected == device_key { "tree-label is-leaf is-active" } else { "tree-label is-leaf" },
                                                                                                        r#type: "button",
                                                                                                        role: "treeitem",
                                                                                                        onclick: move |_| on_select.call(device_key_for_click.clone()),
                                                                                                        strong { "{device.room_name}" }
                                                                                                        small { class: "hint", "{device.factory_no}" }
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn toggle_collapsed(nodes: &mut BTreeSet<String>, key: &str) {
    if !nodes.remove(key) {
        nodes.insert(key.to_string());
    }
}

fn display_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 树节点可以反复展开和收起() {
        let mut collapsed = BTreeSet::new();
        toggle_collapsed(&mut collapsed, "floor:园区/A栋/3楼");
        assert!(collapsed.contains("floor:园区/A栋/3楼"));

        toggle_collapsed(&mut collapsed, "floor:园区/A栋/3楼");
        assert!(!collapsed.contains("floor:园区/A栋/3楼"));
    }
}
