//! 当前 Dioxus 工作台的标准菜单目录。
//!
//! 原项目通过菜单表和 `role_menu` 控制页面访问。迁移后的路由也必须先进入菜单表，
//! 权限管理页面才能像原项目一样为角色配置真实功能，而不是维护前端静态开关。

use std::collections::BTreeMap;

use spacetimedb::{ReducerContext, Table};

use crate::tables::*;

struct CatalogEntry {
    key: &'static str,
    parent_key: Option<&'static str>,
    name: &'static str,
    menu_type: &'static str,
    path: &'static str,
    auth_code: Option<&'static str>,
    internal_only: bool,
}

const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        key: "dashboard",
        parent_key: None,
        name: "运营总览",
        menu_type: "menu",
        path: "/",
        auth_code: Some("dashboard:view"),
        internal_only: false,
    },
    CatalogEntry {
        key: "rental",
        parent_key: None,
        name: "租赁",
        menu_type: "catalog",
        path: "/rental",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "rental.park",
        parent_key: Some("rental"),
        name: "园区管理",
        menu_type: "menu",
        path: "/system/park",
        auth_code: Some("system:park"),
        internal_only: false,
    },
    CatalogEntry {
        key: "rental.park-list",
        parent_key: Some("rental"),
        name: "园区列表",
        menu_type: "menu",
        path: "/rental/list",
        auth_code: Some("rental:list"),
        internal_only: false,
    },
    CatalogEntry {
        key: "rental.factory",
        parent_key: Some("rental"),
        name: "待租厂房",
        menu_type: "menu",
        path: "/rental/factory",
        auth_code: Some("rental:factory"),
        internal_only: false,
    },
    CatalogEntry {
        key: "rental.contract",
        parent_key: Some("rental"),
        name: "合同管理",
        menu_type: "menu",
        path: "/rental/tenant",
        auth_code: Some("rental:tenant"),
        internal_only: false,
    },
    CatalogEntry {
        key: "finance",
        parent_key: None,
        name: "财务管理",
        menu_type: "catalog",
        path: "/finance",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "finance.ledger",
        parent_key: Some("finance"),
        name: "财务流水",
        menu_type: "menu",
        path: "/finance/manage",
        auth_code: Some("finance:manage"),
        internal_only: false,
    },
    CatalogEntry {
        key: "finance.bill",
        parent_key: Some("finance"),
        name: "账单管理",
        menu_type: "menu",
        path: "/bill",
        auth_code: Some("billing:manage"),
        internal_only: false,
    },
    CatalogEntry {
        key: "finance.reimbursement",
        parent_key: Some("finance"),
        name: "报销管理",
        menu_type: "menu",
        path: "/reimbursement/application",
        auth_code: Some("reimbursement:application"),
        internal_only: false,
    },
    CatalogEntry {
        key: "finance.reimbursement-audit",
        parent_key: Some("finance"),
        name: "报销审核",
        menu_type: "menu",
        path: "/reimbursement/audit",
        auth_code: Some("reimbursement:audit"),
        internal_only: false,
    },
    CatalogEntry {
        key: "hrm",
        parent_key: None,
        name: "人事管理",
        menu_type: "catalog",
        path: "/hrm",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "hrm.employee",
        parent_key: Some("hrm"),
        name: "员工档案",
        menu_type: "menu",
        path: "/hrm/information",
        auth_code: Some("hrm:information"),
        internal_only: false,
    },
    CatalogEntry {
        key: "rental.salary",
        // 工资属于人事业务；保留稳定 key，避免同步菜单时产生重复记录。
        parent_key: Some("hrm"),
        name: "工资管理",
        menu_type: "menu",
        path: "/rental/salary",
        auth_code: Some("rental:salary"),
        internal_only: false,
    },
    CatalogEntry {
        key: "hrm.punch",
        parent_key: Some("hrm"),
        name: "考勤打卡",
        menu_type: "menu",
        path: "/hrm/attendance/punch",
        auth_code: Some("hrm:attendance:punch"),
        internal_only: false,
    },
    CatalogEntry {
        key: "hrm.records",
        parent_key: Some("hrm"),
        name: "考勤记录",
        menu_type: "menu",
        path: "/hrm/attendance/stats",
        auth_code: Some("hrm:attendance:stats"),
        internal_only: false,
    },
    CatalogEntry {
        key: "hrm.trajectory",
        parent_key: Some("hrm"),
        name: "轨迹管理",
        menu_type: "menu",
        path: "/hrm/trajectory",
        auth_code: Some("hrm:trajectory"),
        internal_only: false,
    },
    CatalogEntry {
        key: "hrm.leave",
        parent_key: Some("hrm"),
        name: "请假管理",
        menu_type: "menu",
        path: "/hrm/leaveapplication",
        auth_code: Some("hrm:leave"),
        internal_only: false,
    },
    CatalogEntry {
        key: "data-collection",
        parent_key: None,
        name: "设备管理",
        menu_type: "catalog",
        path: "/data-collection",
        auth_code: None,
        internal_only: true,
    },
    // 分组更名为「设备管理」，但模板键与路径保持 `data-collection` / `/data-collection`：
    // 同步逻辑按「模板键或路径」认领已有行，两者一起改会认不出旧行、在生产库里插出
    // 一条重复菜单。这两个值都不出现在界面上，留着旧名字没有代价。
    CatalogEntry {
        key: "device.overview",
        parent_key: Some("data-collection"),
        name: "设备总览",
        menu_type: "menu",
        path: "/device/overview",
        auth_code: Some("device:overview"),
        internal_only: false,
    },
    CatalogEntry {
        key: "meter",
        parent_key: Some("data-collection"),
        name: "智能水电表管理",
        menu_type: "catalog",
        path: "/smart-meter",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "meter.electric",
        parent_key: Some("meter"),
        name: "电表管理",
        menu_type: "menu",
        path: "/smart-meter/meter",
        auth_code: Some("smart-meter:electric"),
        internal_only: false,
    },
    CatalogEntry {
        key: "meter.water",
        parent_key: Some("meter"),
        name: "水表管理",
        menu_type: "menu",
        path: "/smart-meter/water",
        auth_code: Some("smart-meter:water"),
        internal_only: false,
    },
    CatalogEntry {
        key: "device.camera",
        parent_key: Some("data-collection"),
        name: "摄像头管理",
        menu_type: "menu",
        path: "/device/camera",
        auth_code: Some("device:camera"),
        internal_only: false,
    },
    CatalogEntry {
        key: "device.access",
        parent_key: Some("data-collection"),
        name: "门禁设备管理",
        menu_type: "menu",
        path: "/device/access",
        auth_code: Some("device:access"),
        internal_only: false,
    },
    CatalogEntry {
        key: "device.gateway",
        parent_key: Some("data-collection"),
        name: "边缘计算设备",
        menu_type: "menu",
        path: "/device/gateway",
        auth_code: Some("device:gateway"),
        internal_only: false,
    },
    CatalogEntry {
        key: "maintenance",
        parent_key: None,
        name: "维护管理",
        menu_type: "catalog",
        path: "/maintenance",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "maintenance.firefighting",
        parent_key: Some("maintenance"),
        name: "消防管理",
        menu_type: "menu",
        path: "/maintenance/firefighting",
        auth_code: Some("maintenance:firefighting"),
        internal_only: false,
    },
    CatalogEntry {
        key: "maintenance.transformer",
        parent_key: Some("maintenance"),
        name: "变压器管理",
        menu_type: "menu",
        path: "/maintenance/transformer",
        auth_code: Some("maintenance:transformer"),
        internal_only: false,
    },
    CatalogEntry {
        key: "maintenance.elevator",
        parent_key: Some("maintenance"),
        name: "电梯管理",
        menu_type: "menu",
        path: "/maintenance/elevator",
        auth_code: Some("maintenance:elevator"),
        internal_only: false,
    },
    CatalogEntry {
        key: "maintenance.repair-order",
        parent_key: Some("maintenance"),
        name: "报修工单",
        menu_type: "menu",
        path: "/maintenance/repair-order",
        auth_code: Some("maintenance:repair-order"),
        internal_only: false,
    },
    CatalogEntry {
        key: "access",
        parent_key: None,
        name: "门禁管理",
        menu_type: "catalog",
        path: "/access",
        auth_code: Some("access:manage"),
        internal_only: true,
    },
    CatalogEntry {
        key: "access.car",
        parent_key: Some("access"),
        name: "车辆出入管理",
        menu_type: "menu",
        path: "/access/car",
        auth_code: Some("access:car"),
        internal_only: false,
    },
    CatalogEntry {
        key: "access.visitor",
        parent_key: Some("access"),
        name: "访客管理",
        menu_type: "menu",
        path: "/access/visitor",
        auth_code: Some("access:visitor"),
        internal_only: false,
    },
    CatalogEntry {
        key: "access.register",
        parent_key: Some("access"),
        name: "访客登记",
        menu_type: "menu",
        path: "/access/visitor/register",
        auth_code: Some("access:visitor:register"),
        internal_only: false,
    },
    CatalogEntry {
        key: "system",
        parent_key: None,
        name: "系统治理",
        menu_type: "catalog",
        path: "/system",
        auth_code: None,
        internal_only: true,
    },
    CatalogEntry {
        key: "system.role",
        parent_key: Some("system"),
        name: "角色管理",
        menu_type: "menu",
        path: "/system/role",
        auth_code: Some("system:role"),
        internal_only: false,
    },
];

/// 连接现有数据库时补齐缺失菜单；以模板键/路径幂等更新，不删除用户自定义菜单。
pub(crate) fn ensure_default_permission_catalog(ctx: &ReducerContext, customer_id: &str) {
    let mut ids = BTreeMap::<&str, u64>::new();
    for entry in CATALOG {
        let parent_id = entry.parent_key.and_then(|key| ids.get(key).copied());
        let existing = ctx
            .db
            .menu()
            .menu_by_customer()
            .filter(customer_id)
            .find(|menu| {
                menu.template_key.as_deref() == Some(entry.key) || menu.path == entry.path
            });
        let row = if let Some(mut menu) = existing {
            menu.name = entry.name.into();
            menu.menu_type = entry.menu_type.into();
            menu.status = 1;
            menu.path = entry.path.into();
            menu.parent_id = parent_id;
            menu.auth_code = entry.auth_code.map(str::to_string);
            menu.template_key = Some(entry.key.into());
            menu.template_parent_key = entry.parent_key.map(str::to_string);
            menu.template_version = 1;
            menu.template_managed = true;
            menu.template_internal_only = entry.internal_only;
            menu.template_deleted_at = None;
            ctx.db.menu().menu_id().update(menu)
        } else {
            ctx.db.menu().insert(Menu {
                menu_id: 0,
                customer_id: customer_id.into(),
                name: entry.name.into(),
                menu_type: entry.menu_type.into(),
                status: 1,
                path: entry.path.into(),
                active_path: None,
                redirect: None,
                component: None,
                parent_id,
                auth_code: entry.auth_code.map(str::to_string),
                template_key: Some(entry.key.into()),
                template_parent_key: entry.parent_key.map(str::to_string),
                template_version: 1,
                template_managed: true,
                template_internal_only: entry.internal_only,
                template_deleted_at: None,
            })
        };
        ids.insert(entry.key, row.menu_id);
    }
}
