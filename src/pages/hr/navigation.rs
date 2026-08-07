//! 人事模块内的固定业务导航。

use dioxus::prelude::*;

use crate::{
    components::button::{Button, ButtonVariant},
    permissions::{can_access_route, can_manage_hr},
    state::WorkspaceState,
};

#[component]
pub(super) fn HrmNavigation(active: String) -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();
    let codes = state.permission_codes.read();
    let manager = can_manage_hr(&roles, &codes);

    // 每一项都是独立路由，所以用按钮外观的链接而不是 Tabs——
    // Tabs 是同页切换面板，浏览器前进后退会对不上。
    rsx! {
        nav { class: "row", aria_label: "人事管理功能",
            if manager && can_access_route(&roles, &menus, "/hrm/information") {
                Link { to: crate::router::Route::HrmEmployeePage {},
                    Button {
                        variant: if active == "employee" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "员工信息"
                    }
                }
            }
            if can_access_route(&roles, &menus, "/hrm/attendance/punch") {
                Link { to: crate::router::Route::HrmAttendancePunchPage {},
                    Button {
                        variant: if active == "punch" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "考勤打卡"
                    }
                }
            }
            if manager && can_access_route(&roles, &menus, "/hrm/attendance/location") {
                Link { to: crate::router::Route::HrmAttendanceLocationPage {},
                    Button {
                        variant: if active == "location" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        "打卡点设置"
                    }
                }
            }
            if can_access_route(&roles, &menus, "/hrm/attendance/stats") {
                Link { to: crate::router::Route::HrmAttendanceRecordsPage {},
                    Button {
                        variant: if active == "records" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        if manager {
                            "考勤台账"
                        } else {
                            "我的考勤"
                        }
                    }
                }
            }
            if can_access_route(&roles, &menus, "/hrm/trajectory") {
                Link { to: crate::router::Route::HrmTrajectoryPage {},
                    Button {
                        variant: if active == "trajectory" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        if manager {
                            "全员轨迹"
                        } else {
                            "我的轨迹"
                        }
                    }
                }
            }
            if can_access_route(&roles, &menus, "/hrm/leaveapplication") {
                Link { to: crate::router::Route::HrmLeavePage {},
                    Button {
                        variant: if active == "leave" { ButtonVariant::Primary } else { ButtonVariant::Outline },
                        if manager {
                            "请假审批"
                        } else {
                            "我的请假"
                        }
                    }
                }
            }
        }
    }
}
