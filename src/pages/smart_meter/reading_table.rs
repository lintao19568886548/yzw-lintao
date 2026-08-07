//! 智能水电表管理读数表格。

use dioxus::prelude::*;

use crate::{
    components::badge::{Badge, BadgeVariant},
    services::{MeterKind, SmartMeterReading},
};

fn reading_value(value: &str) -> &str {
    if value.trim().is_empty() {
        "--"
    } else {
        value
    }
}

#[component]
pub(super) fn MeterReadingList(kind: MeterKind, rows: Vec<SmartMeterReading>) -> Element {
    let empty_colspan = if kind == MeterKind::Electric { 10 } else { 6 };
    rsx! {
        div { class: "table-shell",
            table { class: "table",
                thead {
                    tr {
                        th { "设备地址" }
                        th { "设备编号" }
                        th {
                            if kind == MeterKind::Electric {
                                "总用电量(度)"
                            } else {
                                "总用水量(吨)"
                            }
                        }
                        if kind == MeterKind::Electric {
                            th { "尖" }
                            th { "峰" }
                            th { "平" }
                            th { "谷" }
                        }
                        th { "冻结类型" }
                        th { "冻结时间" }
                        th { "写入时间" }
                    }
                }
                tbody {
                    if rows.is_empty() {
                        tr {
                            td { class: "table-empty", colspan: empty_colspan, "该日期暂无日冻结数据" }
                        }
                    } else {
                        for row in rows.iter() {
                            tr { key: "reading-{row.com_address}-{row.freeze_time}",
                                td {
                                    div { class: "stack-tight",
                                        strong { "{row.room_name}" }
                                        small { class: "hint", "房间 {row.room_id}" }
                                    }
                                }
                                td { class: "is-mono", "{row.com_address}" }
                                td { class: "is-mono", "{reading_value(&row.data_value)}" }
                                if kind == MeterKind::Electric {
                                    td { class: "is-mono", "{reading_value(&row.data_value_tip)}" }
                                    td { class: "is-mono", "{reading_value(&row.data_value_peak)}" }
                                    td { class: "is-mono", "{reading_value(&row.data_value_flat)}" }
                                    td { class: "is-mono", "{reading_value(&row.data_value_valley)}" }
                                }
                                td {
                                    Badge { variant: BadgeVariant::Secondary, "日冻结" }
                                }
                                td { class: "hint", "{reading_value(&row.freeze_time)}" }
                                td { class: "hint", "{reading_value(&row.write_time)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
