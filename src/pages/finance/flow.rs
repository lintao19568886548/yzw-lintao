//! 财务模块的数据流向总览。
//!
//! 跟"租赁""人事"两个总览不一样，这个模块里大部分关系是**真外键**：
//! 账单（`AmountBill`）必填 `finance_id`，水电明细必填 `bill_id`，报销
//! （`Reimbursement`）也有一个可选的 `finance_id`——这条外键是
//! SpacetimeDB 迁移时特意加上的，原 MySQL 系统靠备注字符串模糊查找
//! 报销对应的流水，这次改成了真正的关系字段。三条链路都画成"从子表
//! 指向父表"的方向：水电明细 / 账单 → 财务流水 ← 报销。
//!
//! "角色管理"和这里一样，虽然口径干净，但没有一个模块把它们摆在
//! 一起统计——账单和报销汇入同一张流水表，是两条独立的来源，不是
//! 顺序关系，所以各自成一条链路。

use dioxus::prelude::*;

use super::model::{
    bill_finance_link_counts, reimbursement_finance_link_counts, utility_bill_link_counts,
};
use crate::{
    components::{
        card::{Card, CardContent},
        data_flow::DataFlowStage,
        Icon,
    },
    permissions::can_access_route,
    router::Route,
    state::WorkspaceState,
};

#[component]
pub fn FinanceOverviewPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();

    let finances = (state.finances)();
    let amount_bills = (state.amount_bills)();
    let ele_bills = (state.ele_bills)();
    let water_bills = (state.water_bills)();
    let reimbursements = (state.reimbursements)();

    let bill_count = amount_bills.len();
    let ele_bill_count = ele_bills.len();
    let water_bill_count = water_bills.len();
    let (ele_linked, ele_orphaned) = utility_bill_link_counts(
        &ele_bills.iter().map(|row| row.bill_id).collect::<Vec<_>>(),
        &amount_bills,
    );
    let (water_linked, water_orphaned) = utility_bill_link_counts(
        &water_bills.iter().map(|row| row.bill_id).collect::<Vec<_>>(),
        &amount_bills,
    );

    let (bill_linked, bill_orphaned) = bill_finance_link_counts(&amount_bills, &finances);
    let finance_count = finances.iter().filter(|row| !row.is_deleted).count();

    let reimbursement_count = reimbursements.iter().filter(|row| !row.is_deleted).count();
    let (reimbursement_linked, reimbursement_pending) =
        reimbursement_finance_link_counts(&reimbursements, &finances);

    let finance_link = can_access_route(&roles, &menus, "/finance/manage")
        .then_some(Route::FinanceManagementPage {});
    let bill_link = can_access_route(&roles, &menus, "/bill").then_some(Route::BillManagementPage {});
    let reimbursement_link = can_access_route(&roles, &menus, "/reimbursement/application")
        .then_some(Route::ReimbursementApplicationPage {});

    let max_count = [
        bill_count,
        ele_bill_count.max(water_bill_count),
        finance_count,
        reimbursement_count,
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
    .max(1);

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "财务总览" }
                    p { class: "page-subtitle",
                        "账单和报销是流入财务流水的两条独立来源，都通过真实外键关联——不是顺序关系，只是共同汇入同一张流水表。"
                    }
                }
            }
            section { class: "section",
                Card {
                    CardContent {
                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title", "水电明细与账单：都是必填外键，一路可追溯" }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "水电明细",
                                    caption: format!(
                                        "电费{ele_linked}挂靠·{ele_orphaned}对不上 / 水费{water_linked}挂靠·{water_orphaned}对不上",
                                    ),
                                    count: ele_bill_count.max(water_bill_count),
                                    max_count,
                                    to: None,
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "账单",
                                    caption: format!("{bill_count} 份账单"),
                                    count: bill_count,
                                    max_count,
                                    to: bill_link.clone(),
                                }
                            }
                        }

                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title",
                                "账单 → 财务流水：必填外键，账单一定会生成一条流水"
                            }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "账单",
                                    caption: format!("{bill_count} 份账单"),
                                    count: bill_count,
                                    max_count,
                                    to: bill_link,
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "财务流水",
                                    caption: format!("{bill_linked} 挂靠账单 · {bill_orphaned} 流水缺失或已删除"),
                                    count: finance_count,
                                    max_count,
                                    to: finance_link.clone(),
                                }
                            }
                        }

                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title",
                                "报销 → 财务流水：可选外键，审批通过前本来就不该有关联流水"
                            }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "报销",
                                    caption: format!("{reimbursement_count} 份有效报销"),
                                    count: reimbursement_count,
                                    max_count,
                                    to: reimbursement_link,
                                }
                                div { class: "data-flow-arrow", Icon { name: "chevron".to_string() } }
                                DataFlowStage {
                                    label: "财务流水",
                                    caption: format!("{reimbursement_linked} 已生成流水 · {reimbursement_pending} 待生成"),
                                    count: finance_count,
                                    max_count,
                                    to: finance_link,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
