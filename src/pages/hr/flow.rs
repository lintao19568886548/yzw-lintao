//! 人事模块的数据流向总览。
//!
//! 三条链路都从"员工"出发，但都不是靠 `employee_id` 直接串起来的：
//! `Employee.user_id` 是可选的软关联（有没有配登录账号），考勤、请假、
//! 定位打卡三张表也都只认 `user_id`，不认 `employee_id`。员工与这些
//! 记录能不能对上，取决于这个员工有没有绑定账号。
//!
//! "工资管理"和"角色管理"虽然跟"人事"摆在同一个侧边栏分组里，但没有画
//! 进这个总览——工资表（`Salary`）用 `rental_tenant_id` 挂在租赁客户
//! 下，跟员工没有任何字段关联；角色管理是权限模块的数据，跟员工档案也
//! 不是一回事。同一个导航分组不等于同一份数据。

use std::collections::BTreeSet;

use dioxus::prelude::*;

use super::model::{employee_binding_counts, linked_id_counts};
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
pub fn HrOverviewPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();

    let employees = (state.employees)();
    let attendances = (state.attendances)();
    let leave_applications = (state.leave_applications)();

    let (bound_count, unbound_count) = employee_binding_counts(&employees);
    let bound_user_ids = employees
        .iter()
        .filter(|employee| !employee.is_deleted)
        .filter_map(|employee| employee.user_id)
        .collect::<BTreeSet<_>>();

    let attendance_count = attendances.len();
    let (attendance_linked, attendance_unlinked) =
        linked_id_counts(attendances.iter().map(|row| row.user_id), &bound_user_ids);

    let leave_count = leave_applications.len();
    let (leave_linked, leave_unlinked) = linked_id_counts(
        leave_applications.iter().map(|row| row.user_id),
        &bound_user_ids,
    );
    let pending_leave = leave_applications
        .iter()
        .filter(|row| row.status == 0)
        .count();

    let employee_link =
        can_access_route(&roles, &menus, "/hrm/information").then_some(Route::HrmEmployeePage {});
    let attendance_link = can_access_route(&roles, &menus, "/hrm/attendance/stats")
        .then_some(Route::HrmAttendanceRecordsPage {});
    let leave_link =
        can_access_route(&roles, &menus, "/hrm/leaveapplication").then_some(Route::HrmLeavePage {});

    let max_count = [bound_count + unbound_count, attendance_count, leave_count]
        .into_iter()
        .max()
        .unwrap_or(0)
        .max(1);

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "人事总览" }
                    p { class: "page-subtitle",
                        "员工档案、考勤和请假各自独立统计——它们靠登录账号（user_id）互相关联，不是数据库外键；没绑定账号的员工，他的考勤和请假永远对不上号。"
                    }
                }
            }
            section { class: "section",
                Card {
                    CardContent {
                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title", "员工与登录账号：软关联，不是每个员工都配了账号" }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "员工",
                                    caption: format!("{bound_count} 已绑定账号 · {unbound_count} 未绑定"),
                                    count: bound_count + unbound_count,
                                    max_count,
                                    to: employee_link,
                                }
                            }
                        }

                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title", "考勤：按登录账号归属，未绑定账号的员工不会有考勤记录" }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "已绑定账号",
                                    caption: format!("{bound_count} 名员工"),
                                    count: bound_count,
                                    max_count,
                                    to: None,
                                }
                                div { class: "data-flow-arrow is-inferred", title: "按 user_id 关联，数据库里没有指向员工表的外键",
                                    Icon { name: "link".to_string() }
                                }
                                DataFlowStage {
                                    label: "考勤记录",
                                    caption: format!("{attendance_linked} 关联在职员工 · {attendance_unlinked} 对不上员工"),
                                    count: attendance_count,
                                    max_count,
                                    to: attendance_link,
                                }
                            }
                        }

                        div { class: "data-flow-group",
                            p { class: "data-flow-group-title", "请假：同样按登录账号归属，不认员工档案本身" }
                            div { class: "data-flow",
                                DataFlowStage {
                                    label: "已绑定账号",
                                    caption: format!("{bound_count} 名员工"),
                                    count: bound_count,
                                    max_count,
                                    to: None,
                                }
                                div { class: "data-flow-arrow is-inferred", title: "按 user_id 关联，数据库里没有指向员工表的外键",
                                    Icon { name: "link".to_string() }
                                }
                                DataFlowStage {
                                    label: "请假申请",
                                    caption: format!("{leave_linked} 关联在职员工 · {leave_unlinked} 对不上员工"),
                                    count: leave_count,
                                    max_count,
                                    to: leave_link,
                                    div { class: "data-flow-split",
                                        span { class: "data-flow-pill", "{pending_leave} 待审批" }
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
