//! 数据地图：全库业务表的数量、关联方式与孤儿数据的一张图。
//!
//! 园区名下的物理资产有两支：厂房和宿舍，两支都有逐层记录。水电表是
//! 第三类资产——它装在某一层，也可能装在园区公共区域（生产账单里的
//! 「公共用电」就是这一类），所以它的必填外键是园区而不是楼层。
//!
//! 水电单价不在表上而在合同上：同一块表换一户就换一个价，报价是合同
//! 条款。合同 →(关联表 + 约定单价)→ 水电表 这条边画的就是这件事。
//!
//! 布局以园区为中心放射——数据库里厂房、合同、财务流水、门禁、维护设备
//! 全都直接挂在 `park_id` 下，园区是事实上的中心节点。人事那一簇
//! （员工/工资/考勤/请假）与园区没有任何字段关联，画成右下角的独立小岛，
//! 这个"不相连"本身就是要传达的事实。
//!
//! 工资曾经挂在租赁合同下（原 MySQL 结构如此），页面上因此出现「给某某
//! 有限公司发工资」这种记录；已改为归属员工，节点也随之移进人事岛。
//!
//! 合同与财务之间没有直接字段关联，靠账单中转：
//! 合同 →(tenant_id)→ 租金账单 →(finance_id)→ 财务流水。
//!
//! 维护四表还有可选的 `factory_id` 次级关联，为避免连线交叉没有画出。
//!
//! 智能水电表管理自身没有 SpacetimeDB 表（纯 HTTP 通道），所以图上没有它的节点；
//! 水电表节点数的是本库台账，点击跳到设备管理页——台账维护在园区详情里，但
//! 天天要看这些表的人是去读数的。

use std::collections::BTreeSet;

use dioxus::prelude::*;

use super::model::{
    format_area_square_metres, meter_location_counts, optional_link_counts, sentinel_link_counts,
};
use crate::{
    components::card::{Card, CardContent},
    pages::{
        bill_finance_link_counts, employee_binding_counts, factory_park_link_counts,
        floor_factory_link_counts, is_active_income_contract, linked_id_counts,
        reimbursement_finance_link_counts, unmatched_contract_count, utility_bill_link_counts,
    },
    permissions::can_access_route,
    router::Route,
    services::park_ref,
    state::WorkspaceState,
};

// 节点中心坐标（viewBox 0 0 1240 880）。表结构是静态的，手工布点即可，
// 不值得为一张固定的图引入自动布局。
const PARK: (f64, f64) = (480.0, 430.0);
const FACTORY: (f64, f64) = (260.0, 270.0);
const METER: (f64, f64) = (250.0, 430.0);
const FLOOR: (f64, f64) = (110.0, 150.0);
const DORMITORY: (f64, f64) = (110.0, 340.0);
const CONTRACT: (f64, f64) = (480.0, 170.0);
const PROFILE: (f64, f64) = (280.0, 80.0);
const SALARY: (f64, f64) = (860.0, 780.0);
const FINANCE: (f64, f64) = (770.0, 430.0);
const BILL: (f64, f64) = (960.0, 340.0);
const ELE_BILL: (f64, f64) = (1140.0, 260.0);
const WATER_BILL: (f64, f64) = (1140.0, 420.0);
const REIMBURSE: (f64, f64) = (700.0, 270.0);
const CAR: (f64, f64) = (220.0, 520.0);
const VISITOR: (f64, f64) = (220.0, 650.0);
const FIREFIGHTING: (f64, f64) = (380.0, 670.0);
const TRANSFORMER: (f64, f64) = (530.0, 710.0);
const ELEVATOR: (f64, f64) = (680.0, 670.0);
const REPAIR: (f64, f64) = (890.0, 480.0);
const EMPLOYEE: (f64, f64) = (1000.0, 650.0);
const ATTENDANCE: (f64, f64) = (1000.0, 780.0);
const LEAVE: (f64, f64) = (1140.0, 780.0);

#[derive(Clone, Copy, PartialEq)]
enum EdgeKind {
    /// 必填外键：字段非空且理应始终指向有效父级。
    Required,
    /// 可选外键或 0 哨兵：允许"未分配"状态。
    Optional,
    /// 软关联：靠文本（姓名+电话）或账号 id 匹配，数据库里没有这条外键，
    /// 但至少能程序化算出对得上多少条。
    Soft,
}

#[component]
pub fn DataMapPage() -> Element {
    let state = use_context::<WorkspaceState>();
    let roles = state.roles.read();
    let menus = state.menus.read();

    let parks = (state.parks)();
    let factories = (state.factories)();
    let floors = (state.factory_floors)();
    let dormitories = (state.dormitories)();
    let meters = (state.utility_meters)();
    let tenant_meters = (state.rental_tenant_meters)();
    let contracts = (state.rental_tenants)();
    let tenant_floors = (state.rental_tenant_floors)();
    let profiles = (state.tenant_profiles)();
    let salaries = (state.salaries)();
    let finances = (state.finances)();
    let bills = (state.amount_bills)();
    let ele_bills = (state.ele_bills)();
    let water_bills = (state.water_bills)();
    let reimbursements = (state.reimbursements)();
    let employees = (state.employees)();
    let attendances = (state.attendances)();
    let leaves = (state.leave_applications)();
    let cars = (state.access_cars)();
    let visitors = (state.access_visitors)();
    let firefightings = (state.firefighting_assets)();
    let transformers = (state.transformer_assets)();
    let elevators = (state.elevator_assets)();
    let repairs = (state.repair_orders)();

    let active_park_ids = parks
        .iter()
        .filter(|park| !park.is_deleted)
        .map(|park| park.park_id)
        .collect::<BTreeSet<_>>();
    let park_count = active_park_ids.len();

    let (factory_linked, factory_orphaned) = factory_park_link_counts(&factories, &parks);
    let (floor_linked, floor_orphaned) = floor_factory_link_counts(&floors, &factories);
    // 宿舍跟厂房一样直接挂在园区下，是园区资产的另一支。它没有逐层记录，
    // 楼层数和房间数压在建筑这一行里（floor_count / total_rooms），
    // 所以合同暂时无法像厂房那样精确关联到某一层。
    let live_dormitories = dormitories
        .iter()
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    let dormitory_count = live_dormitories.len();
    let (_, dormitory_orphaned) = linked_id_counts(
        live_dormitories.iter().map(|row| Some(row.park_id)),
        &active_park_ids,
    );

    // 水电表的必填外键是园区：装在哪一层是可选的，公共区域的表不挂层。
    let live_meters = meters
        .iter()
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    let (_, meter_orphaned) =
        sentinel_link_counts(live_meters.iter().map(|row| row.park_id), &active_park_ids);
    let (meter_on_factory, meter_on_dormitory, meter_public) = meter_location_counts(
        live_meters
            .iter()
            .map(|row| (row.factory_floor_id, row.dormitory_floor_id)),
    );

    let active_contracts = contracts
        .iter()
        .filter(|row| is_active_income_contract(row))
        .collect::<Vec<_>>();
    let contract_count = active_contracts.len();
    let (_, contract_orphaned) = sentinel_link_counts(
        active_contracts.iter().map(|row| row.park_id),
        &active_park_ids,
    );
    let unmatched_contracts = unmatched_contract_count(&contracts, &profiles);

    let profile_count = profiles
        .iter()
        .filter(|row| row.status == 1 && !row.is_deleted)
        .count();

    // 合同↔楼层已经是真实关联表了。回填完成前大部分历史合同还没挂上，
    // 所以这里统计"已关联/未关联"，让补录进度在图上直接可见。
    let linked_contract_ids = tenant_floors
        .iter()
        .map(|link| link.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let contract_with_floor = active_contracts
        .iter()
        .filter(|row| linked_contract_ids.contains(&row.rental_tenant_id))
        .count();
    let contract_without_floor = contract_count.saturating_sub(contract_with_floor);

    // 有几份合同已经把水电单价谈进系统里。没有关联表之前这个数字恒等于
    // 零——单价只存在于每月开账单时手敲的那一格。
    let priced_contract_ids = tenant_meters
        .iter()
        .map(|link| link.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let contract_with_meter = active_contracts
        .iter()
        .filter(|row| priced_contract_ids.contains(&row.rental_tenant_id))
        .count();

    // 楼层已用面积已经改由合同关联算出，人工维护的字段不复存在。剩下两个
    // 口径仍值得并列：合同自己填报的面积，和它实际选定楼层后分配到的面积。
    // 两者对不上说明这份合同的楼层还没选全，或者填报面积本身就不准。
    let contract_area: i64 = active_contracts
        .iter()
        .filter_map(|row| row.area_centi_square_metres)
        .filter(|area| *area > 0)
        .sum();
    let linked_area: i64 = tenant_floors
        .iter()
        .map(|link| link.area_centi_square_metres)
        .sum();

    let active_tenant_ids = contracts
        .iter()
        .filter(|row| !row.is_deleted)
        .map(|row| row.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let live_salaries = salaries
        .iter()
        .filter(|row| !row.is_deleted)
        .collect::<Vec<_>>();
    let salary_count = live_salaries.len();
    let active_employee_ids = employees
        .iter()
        .filter(|row| !row.is_deleted)
        .map(|row| row.employee_id)
        .collect::<BTreeSet<_>>();
    let (_, salary_orphaned) = linked_id_counts(
        live_salaries.iter().map(|row| Some(row.employee_id)),
        &active_employee_ids,
    );

    let finance_count = finances.iter().filter(|row| !row.is_deleted).count();
    let (_, finance_orphaned) = sentinel_link_counts(
        finances
            .iter()
            .filter(|row| !row.is_deleted)
            .map(|row| row.park_id),
        &active_park_ids,
    );

    let (_, bill_orphaned) = bill_finance_link_counts(&bills, &finances);
    // 账单是连接合同与财务的桥：合同 →(tenant_id)→ 账单 →(finance_id)→ 流水。
    // Finance 表本身没有指向合同的字段，"这笔钱是哪份合同产生的"只能绕道账单回答。
    let (bill_with_contract, bill_without_contract) =
        sentinel_link_counts(bills.iter().map(|row| row.tenant_id), &active_tenant_ids);
    let ele_ids = ele_bills.iter().map(|row| row.bill_id).collect::<Vec<_>>();
    let water_ids = water_bills
        .iter()
        .map(|row| row.bill_id)
        .collect::<Vec<_>>();
    let (_, ele_orphaned) = utility_bill_link_counts(&ele_ids, &bills);
    let (_, water_orphaned) = utility_bill_link_counts(&water_ids, &bills);

    let reimbursement_count = reimbursements.iter().filter(|row| !row.is_deleted).count();
    let (_, reimbursement_pending) = reimbursement_finance_link_counts(&reimbursements, &finances);

    let (car_linked, car_unassigned, car_dangling) = optional_link_counts(
        cars.iter().map(|row| park_ref(row.park_id)),
        &active_park_ids,
    );
    let (visitor_linked, visitor_unassigned, visitor_dangling) = optional_link_counts(
        visitors.iter().map(|row| park_ref(row.park_id)),
        &active_park_ids,
    );

    let (_, fire_orphaned) = linked_id_counts(
        firefightings.iter().map(|row| Some(row.park_id)),
        &active_park_ids,
    );
    let (_, transformer_orphaned) = linked_id_counts(
        transformers.iter().map(|row| Some(row.park_id)),
        &active_park_ids,
    );
    let (_, elevator_orphaned) = linked_id_counts(
        elevators.iter().map(|row| Some(row.park_id)),
        &active_park_ids,
    );
    let (_, repair_orphaned) = linked_id_counts(
        repairs.iter().map(|row| Some(row.park_id)),
        &active_park_ids,
    );

    let (bound_employees, unbound_employees) = employee_binding_counts(&employees);
    let bound_user_ids = employees
        .iter()
        .filter(|row| !row.is_deleted)
        .filter_map(|row| row.user_id)
        .collect::<BTreeSet<_>>();
    let (_, attendance_unlinked) =
        linked_id_counts(attendances.iter().map(|row| row.user_id), &bound_user_ids);
    let (_, leave_unlinked) =
        linked_id_counts(leaves.iter().map(|row| row.user_id), &bound_user_ids);

    let route_if =
        |path: &str, route: Route| can_access_route(&roles, &menus, path).then_some(route);
    let manage_link = route_if("/rental/manage", Route::RentalManagementPage {});
    let factory_link = route_if("/rental/factory", Route::VacantFactoryPage {});
    // 水电表节点跳智能水电表管理而不是园区管理：台账维护在园区详情里，但真正
    // 天天要看这些表的人是去读数的，设备管理页才是他们的落点。
    let meter_link = route_if("/smart-meter/meter", Route::SmartElectricMeterPage {});
    let contract_link = route_if("/rental/tenant", Route::ContractManagementPage {});
    let profile_link = route_if("/rental/tenants", Route::TenantManagementPage {});
    let salary_link = route_if("/rental/salary", Route::SalaryManagementPage {});
    let finance_link = route_if("/finance/manage", Route::FinanceManagementPage {});
    let bill_link = route_if("/bill", Route::BillManagementPage {});
    let reimburse_link = route_if(
        "/reimbursement/application",
        Route::ReimbursementApplicationPage {},
    );
    let car_link = route_if("/access/car", Route::AccessCarPage {});
    let visitor_link = route_if("/access/visitor", Route::AccessVisitorPage {});
    let fire_link = route_if(
        "/maintenance/firefighting",
        Route::MaintenanceFirefightingPage { factory: String::new() },
    );
    let transformer_link = route_if(
        "/maintenance/transformer",
        Route::MaintenanceTransformerPage { factory: String::new() },
    );
    let elevator_link = route_if("/maintenance/elevator", Route::MaintenanceElevatorPage { factory: String::new() });
    let repair_link = route_if(
        "/maintenance/repair-order",
        Route::MaintenanceRepairOrderPage {},
    );
    let employee_link = route_if("/hrm/information", Route::HrmEmployeePage {});
    let attendance_link = route_if("/hrm/attendance/stats", Route::HrmAttendanceRecordsPage {});
    let leave_link = route_if("/hrm/leaveapplication", Route::HrmLeavePage {});

    rsx! {
        main { class: "page",
            header { class: "page-header",
                div { class: "page-title",
                    h1 { "数据地图" }
                    p { class: "page-subtitle",
                        "每张业务表的数据量、彼此如何关联、哪些是孤儿数据。以园区为中心——除了人事，所有业务都直接挂在园区之下。"
                    }
                }
            }
            section { class: "section",
                Card {
                    CardContent {
                        div { class: "data-map-scroll",
                            svg { class: "data-map-svg", view_box: "0 0 1240 880",
                                // 人事岛背景框：先画，垫在最底层
                                rect {
                                    class: "map-island",
                                    x: "786", y: "596", width: "428", height: "272", rx: "14",
                                }
                                text { class: "map-island-label", x: "800", y: "618", "人事域 · 与园区无字段关系" }

                                // 边先画、节点后画：节点矩形盖住线头，连线直接取中心点即可
                                MapEdge { from: FACTORY, to: PARK, kind: EdgeKind::Optional }
                                MapEdge { from: FLOOR, to: FACTORY, kind: EdgeKind::Required }
                                MapEdge { from: DORMITORY, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: METER, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: CONTRACT, to: METER, kind: EdgeKind::Optional, label: "关联表 · 约定水电单价", label_offset: -22.0 }
                                MapEdge { from: CONTRACT, to: PARK, kind: EdgeKind::Optional }
                                // 合同租哪一层：多对多关联表，回填期间大部分历史合同还没挂上。
                                MapEdge { from: CONTRACT, to: FLOOR, kind: EdgeKind::Optional, label: "关联表 · 一份合同可跨多层", label_offset: 22.0 }
                                MapEdge { from: PROFILE, to: CONTRACT, kind: EdgeKind::Soft }
                                MapEdge { from: SALARY, to: EMPLOYEE, kind: EdgeKind::Required }
                                MapEdge { from: BILL, to: CONTRACT, kind: EdgeKind::Optional }
                                MapEdge { from: BILL, to: FINANCE, kind: EdgeKind::Required }
                                MapEdge { from: ELE_BILL, to: BILL, kind: EdgeKind::Required }
                                MapEdge { from: WATER_BILL, to: BILL, kind: EdgeKind::Required }
                                MapEdge { from: REIMBURSE, to: FINANCE, kind: EdgeKind::Optional }
                                MapEdge { from: FINANCE, to: PARK, kind: EdgeKind::Optional }
                                MapEdge { from: CAR, to: PARK, kind: EdgeKind::Optional }
                                MapEdge { from: VISITOR, to: PARK, kind: EdgeKind::Optional }
                                MapEdge { from: FIREFIGHTING, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: TRANSFORMER, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: ELEVATOR, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: REPAIR, to: PARK, kind: EdgeKind::Required }
                                MapEdge { from: ATTENDANCE, to: EMPLOYEE, kind: EdgeKind::Soft }
                                MapEdge { from: LEAVE, to: EMPLOYEE, kind: EdgeKind::Soft }

                                MapNode { at: PARK, label: "园区", count: park_count, hub: true, to: manage_link.clone() }
                                MapNode { at: FACTORY, label: "厂房", count: factory_linked + factory_orphaned, orphan: factory_orphaned, to: factory_link.clone() }
                                MapNode { at: FLOOR, label: "楼层", count: floor_linked + floor_orphaned, orphan: floor_orphaned, to: factory_link }
                                MapNode { at: DORMITORY, label: "宿舍", count: dormitory_count, orphan: dormitory_orphaned, note: (dormitory_count == 0).then(|| "台账为空".to_string()), to: manage_link.clone() }
                                MapNode { at: METER, label: "水电表", count: live_meters.len(), orphan: meter_orphaned, note: (!live_meters.is_empty()).then(|| format!("厂房 {meter_on_factory} · 宿舍 {meter_on_dormitory} · 公共 {meter_public}")), to: meter_link }
                                MapNode { at: CONTRACT, label: "租赁合同", count: contract_count, orphan: contract_orphaned, note: (contract_without_floor > 0).then(|| format!("{contract_with_floor} 已关联楼层 · {contract_without_floor} 待补录")), to: contract_link }
                                MapNode { at: PROFILE, label: "客户档案", count: profile_count, note: (unmatched_contracts > 0).then(|| format!("{unmatched_contracts} 份合同未归档")), to: profile_link }
                                MapNode { at: SALARY, label: "工资记录", count: salary_count, orphan: salary_orphaned, to: salary_link }
                                MapNode { at: FINANCE, label: "财务流水", count: finance_count, orphan: finance_orphaned, to: finance_link }
                                MapNode { at: BILL, label: "租金账单", count: bills.len(), orphan: bill_orphaned, note: (bill_without_contract > 0).then(|| format!("{bill_with_contract} 挂合同 · {bill_without_contract} 未挂")), to: bill_link.clone() }
                                MapNode { at: ELE_BILL, label: "电费明细", count: ele_bills.len(), orphan: ele_orphaned, to: bill_link.clone() }
                                MapNode { at: WATER_BILL, label: "水费明细", count: water_bills.len(), orphan: water_orphaned, to: bill_link }
                                MapNode { at: REIMBURSE, label: "报销", count: reimbursement_count, note: (reimbursement_pending > 0).then(|| format!("{reimbursement_pending} 份待生成流水")), to: reimburse_link }
                                MapNode { at: CAR, label: "车辆登记", count: car_linked + car_unassigned + car_dangling, orphan: car_dangling, note: (car_unassigned > 0).then(|| format!("{car_unassigned} 未分配园区")), to: car_link }
                                MapNode { at: VISITOR, label: "访客登记", count: visitor_linked + visitor_unassigned + visitor_dangling, orphan: visitor_dangling, note: (visitor_unassigned > 0).then(|| format!("{visitor_unassigned} 未分配园区")), to: visitor_link }
                                MapNode { at: FIREFIGHTING, label: "消防记录", count: firefightings.len(), orphan: fire_orphaned, to: fire_link }
                                MapNode { at: TRANSFORMER, label: "变压器", count: transformers.len(), orphan: transformer_orphaned, to: transformer_link }
                                MapNode { at: ELEVATOR, label: "电梯", count: elevators.len(), orphan: elevator_orphaned, to: elevator_link }
                                MapNode { at: REPAIR, label: "维修工单", count: repairs.len(), orphan: repair_orphaned, to: repair_link }
                                MapNode { at: EMPLOYEE, label: "员工", count: bound_employees + unbound_employees, note: (unbound_employees > 0).then(|| format!("{unbound_employees} 未绑定账号")), to: employee_link }
                                MapNode { at: ATTENDANCE, label: "考勤", count: attendances.len(), note: (attendance_unlinked > 0).then(|| format!("{attendance_unlinked} 对不上员工")), to: attendance_link }
                                MapNode { at: LEAVE, label: "请假", count: leaves.len(), note: (leave_unlinked > 0).then(|| format!("{leave_unlinked} 对不上员工")), to: leave_link }
                            }
                        }
                        div { class: "data-map-legend",
                            span { class: "legend-item", span { class: "legend-line is-required" } "必填外键" }
                            span { class: "legend-item", span { class: "legend-line is-optional" } "可选外键 / 未分配(0)" }
                            span { class: "legend-item", span { class: "legend-line is-soft" } "软关联（姓名电话 / 账号）" }
                            span { class: "legend-item", span { class: "legend-badge" } "孤儿数据（父级缺失或已删除）" }
                        }
                        if contract_without_floor > 0 {
                            p { class: "notice", role: "status",
                                "合同与楼层的关联表已建立，历史数据仍在补录：{contract_with_floor} 份合同已选定楼层，"
                                "{contract_without_floor} 份还没有（这些合同租哪一层只写在 address 自由文本里，需要人工确认后补选）。"
                                "面积两个口径并列：合同填报 {format_area_square_metres(contract_area)} ㎡ · "
                                "已按楼层分配 {format_area_square_metres(linked_area)} ㎡。"
                                "楼层的已用面积和出租状态现在都由合同关联算出，不再有人工维护的字段。"
                            }
                        }
                        if contract_with_meter < contract_count {
                            p { class: "notice", role: "status",
                                "水电定价刚刚从账单里搬回合同：{contract_with_meter} 份合同已约定水电单价，"
                                "{contract_count - contract_with_meter} 份还没有。在此之前单价只存在于每月开账单时手敲的那一格，"
                                "同一份合同连开十二个月就要敲十二遍；电费明细里 262 条表计名为空、"
                                "91 条把尖峰平谷的时段当表名填，都是因为没有地方放这些信息。"
                            }
                        }
                        p { class: "hint",
                            "维护设备的可选厂房归属未画出以保持图面清晰；水电表装在哪一层是可选的（公共区域的表不挂层），图上只画它的必填园区归属。智能水电表管理走外部接口、自身没有本库数据表，图上的水电表节点数的是本库台账，点击可跳转到设备管理下的水电表页。有权限的节点可以点击进入对应业务页面。"
                        }
                    }
                }
            }
        }
    }
}

/// 图上的一个表节点：矩形 + 表名 + 行数，右上角孤儿角标，下方浅色标注。
#[component]
fn MapNode(
    at: (f64, f64),
    label: &'static str,
    count: usize,
    #[props(default = 0)] orphan: usize,
    #[props(default)] note: Option<String>,
    #[props(default)] to: Option<Route>,
    /// 中心节点（园区）加强调描边，一眼看出放射的圆心。
    #[props(default = false)]
    hub: bool,
) -> Element {
    let (x, y) = at;
    let navigator = navigator();
    let clickable = to.is_some();
    let class = match (clickable, hub) {
        (true, true) => "map-node is-hub is-link",
        (true, false) => "map-node is-link",
        (false, true) => "map-node is-hub",
        (false, false) => "map-node",
    };
    rsx! {
        g {
            class: "{class}",
            onclick: move |_| {
                if let Some(route) = to.clone() {
                    navigator.push(route);
                }
            },
            rect {
                class: "map-node-box",
                x: "{x - 60.0}", y: "{y - 26.0}", width: "120", height: "52", rx: "10",
            }
            text { class: "map-node-label", x: "{x}", y: "{y - 5.0}", text_anchor: "middle", "{label}" }
            text { class: "map-node-count", x: "{x}", y: "{y + 17.0}", text_anchor: "middle", "{count}" }
            if orphan > 0 {
                rect {
                    class: "map-orphan-box",
                    x: "{x + 16.0}", y: "{y - 35.0}", width: "46", height: "18", rx: "9",
                }
                text { class: "map-orphan-text", x: "{x + 39.0}", y: "{y - 22.0}", text_anchor: "middle", "孤 {orphan}" }
            }
            if let Some(note) = note {
                text { class: "map-node-note", x: "{x}", y: "{y + 42.0}", text_anchor: "middle", "{note}" }
            }
        }
    }
}

/// 两个节点中心之间的关系连线，线型由关系种类决定。
///
/// `label` 只给需要额外解释的连线用（目前是未建模那条）；普通外键连线
/// 靠图例就能读懂，逐条标注反而糊。
#[component]
fn MapEdge(
    from: (f64, f64),
    to: (f64, f64),
    kind: EdgeKind,
    #[props(default)] label: Option<&'static str>,
    /// 标签相对连线中点的纵向偏移，避免压在线上或撞到别的节点。
    #[props(default = -10.0)]
    label_offset: f64,
) -> Element {
    let class = match kind {
        EdgeKind::Required => "map-edge is-required",
        EdgeKind::Optional => "map-edge is-optional",
        EdgeKind::Soft => "map-edge is-soft",
    };
    let mid_x = (from.0 + to.0) / 2.0;
    let mid_y = (from.1 + to.1) / 2.0 + label_offset;
    rsx! {
        line {
            class: "{class}",
            x1: "{from.0}", y1: "{from.1}", x2: "{to.0}", y2: "{to.1}",
        }
        if let Some(label) = label {
            text {
                class: "map-edge-label",
                x: "{mid_x}", y: "{mid_y}", text_anchor: "middle",
                "{label}"
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[component]
    fn DataMapRenderTestRoot() -> Element {
        let state = crate::app::use_workspace_state();
        use_context_provider(|| state);
        rsx! { DataMapPage {} }
    }

    /// 页面在空数据下要能完成首次渲染——SVG 属性写错（比如 rsx 不认识的
    /// 属性名）在这里就会崩，不用等到浏览器里才发现。
    #[test]
    fn 数据地图页面可以完成首次渲染() {
        let html = dioxus_ssr::render_element(rsx! { DataMapRenderTestRoot {} });
        assert!(html.contains("数据地图"), "页面标题没有进入渲染结果");
        assert!(
            html.contains("viewBox=\"0 0 1240 880\""),
            "SVG 画布没有进入渲染结果"
        );
        assert!(html.contains("人事域"), "人事岛标注没有进入渲染结果");
        assert!(html.contains("必填外键"), "图例没有进入渲染结果");
        assert!(
            html.contains("关联表 · 一份合同可跨多层"),
            "合同与楼层的关联说明没有进入渲染结果"
        );
    }
}
