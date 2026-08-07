//! 静态业务页面与 SpacetimeDB 动态菜单之间的统一授权规则。

use crate::spacetime_bindings::{menu_type::Menu, role_type::Role};

/// 判断当前角色是否包含启用的系统 Super。
pub fn is_system_super(roles: &[Role]) -> bool {
    roles
        .iter()
        .any(|role| role.status == 1 && role.name == "Super" && role.scope == "system")
}

/// 将已经迁移为静态 Dioxus 页面、但仍沿用原菜单路径的地址归一化。
fn equivalent_paths(path: &str) -> &[&str] {
    if path.starts_with("/rental/factory/detail/") {
        return &["/rental/factory", "/rental/list"];
    }
    // 合同表单从弹窗改成整页之后多了三条路由，权限仍然跟着合同管理菜单走——
    // 能进合同列表就能开表单，页面自己再按 readonly 区分查看和编辑。
    if path == "/rental/tenant/new"
        || path.starts_with("/rental/tenant/edit/")
        || path.starts_with("/rental/tenant/detail/")
    {
        return &["/rental/tenant"];
    }
    if path.starts_with("/rental/detail/") {
        // 园区档案页原来只从园区列表进入，列表页删除后入口改到园区管理。
        // 两个菜单都保留：库里的 /rental/list 菜单行还在，只认一个会让现有
        // 角色里的某一半失去访问权。
        return &["/rental/list", "/rental/manage", "/system/park"];
    }
    match path {
        // 账期结转是账单的下游动作：能看账单就能看结转清单。真正的边界在
        // Reducer——确认要园区管理权限，看得见不等于点得动。
        "/bill/carryover" => &["/bill"],
        "/rental/manage" => &["/rental/manage", "/system/park"],
        // 待租厂房的入口在侧边栏是按 /rental/list 判定的（园区列表页已删除，
        // 但菜单行还在库里，改判别的路径会让现有角色掉权）。路由守卫必须接受
        // 同一条菜单，否则只有 /rental/list 的角色会看见入口、点进去却被拦下，
        // 而只有 /rental/factory 的角色反过来看不见入口。两处认同一组菜单。
        "/rental/factory" => &["/rental/factory", "/rental/list"],
        // 打卡点是考勤规则的设置面，跟着员工信息那张菜单走——能维护员工的人
        // 才该能改「只准在哪儿打卡」。单独配一张菜单反而会让现有角色全都看不到。
        "/hrm/attendance/location" => &["/hrm/information"],
        "/rental/tenants" => &["/rental/tenant"],
        // 总览页是租赁模块所有二级页面的汇总只读视图，不对应任何一条
        // 具体菜单——只要账号能看见租赁模块里的任意一个业务页面，就该
        // 能看这份汇总，不需要管理员单独为它建一条新菜单授权。
        "/rental/overview" => &[
            "/rental/manage",
            "/system/park",
            "/rental/list",
            "/rental/factory",
            "/rental/tenant",
        ],
        // 同样是只读汇总：只要能看见“人事”这个侧边栏分组下的任意一个
        // 页面（人事、工资、角色管理），就该能看这份汇总——不因为汇总页
        // 本身没配菜单而把整个分组都能看的账号挡在外面。
        "/hrm/overview" => &[
            "/hrm/information",
            "/hrm/attendance/punch",
            "/hrm/attendance/stats",
            "/hrm/trajectory",
            "/hrm/leaveapplication",
            "/rental/salary",
            "/system/role",
        ],
        // 同样是只读汇总：只要能看见“财务”分组下的任意一个页面
        // （财务管理、账单管理、报销管理/审核），就该能看这份汇总。
        "/finance/overview" => &[
            "/finance/manage",
            "/bill",
            "/reimbursement/application",
            "/reimbursement/audit",
        ],
        // 设备总览与上面几个汇总页同理：它自己有一条菜单（device:overview），
        // 管理员可以单独授予；但只要能看见设备管理分组下的任意一页，就该能看
        // 这份汇总，否则新加的这条菜单没授之前整个分组看上去毫无变化。
        // 它只显示台数，行级范围仍由服务端 my_* 视图按园区约束。
        "/device/overview" => &[
            "/device/overview",
            "/device/camera",
            "/device/access",
            "/device/gateway",
            "/smart-meter/meter",
            "/smart-meter/water",
        ],
        // 数据地图是跨全部模块的只读汇总：能进运营总览或任何一个业务
        // 模块页面的账号都可以看。行级数据范围仍由服务端 my_* 视图
        // 按园区/本人约束，这里只决定"这一页可不可以打开"。
        "/data-map" => &[
            "/",
            "/rental/manage",
            "/system/park",
            "/rental/list",
            "/rental/factory",
            "/rental/tenant",
            "/rental/salary",
            "/finance/manage",
            "/bill",
            "/reimbursement/application",
            "/reimbursement/audit",
            "/hrm/information",
            "/access/car",
            "/access/visitor",
            "/maintenance/firefighting",
            "/maintenance/transformer",
            "/maintenance/elevator",
            "/maintenance/repair-order",
        ],
        _ => &[],
    }
}

/// 页面路径是否由当前账号的 `my_menus` 明确授权。
///
/// 系统 Super 拥有全部页面；其他账号必须匹配启用菜单的 `path` 或 `active_path`。
pub fn can_access_route(roles: &[Role], menus: &[Menu], path: &str) -> bool {
    if is_system_super(roles) {
        return true;
    }
    // 扫码巡检页对任何已登录账号放行：巡检权限由权限码（maintenance:inspect）
    // 承载而非菜单，路由层看不到权限码。页面自己提示无权限，服务端 Reducer
    // 是真正的边界（docs/变压器台账与扫码巡检.md §3.4）。
    if path.starts_with("/maintenance/inspect/") {
        return true;
    }
    // 个人中心同理对任何已登录账号放行：维护自己的头像与姓名不是业务权限，
    // 服务端 Reducer 只允许改登录者本人的行。
    if path == "/profile" {
        return true;
    }
    let aliases = equivalent_paths(path);
    menus.iter().any(|menu| {
        if menu.status != 1 || menu.template_deleted_at.is_some() || menu.template_internal_only {
            return false;
        }
        menu.path == path
            || menu.active_path.as_deref() == Some(path)
            || aliases
            .iter()
            .any(|alias| menu.path == *alias || menu.active_path.as_deref() == Some(*alias))
    })
}

/// 返回当前账号可以进入的第一个页面，用作无权访问提示中的返回地址。
pub fn first_accessible_path(roles: &[Role], menus: &[Menu]) -> String {
    if is_system_super(roles) || can_access_route(roles, menus, "/") {
        return "/".into();
    }
    menus
        .iter()
        .filter(|menu| {
            menu.status == 1
                && menu.template_deleted_at.is_none()
                && !menu.template_internal_only
                && !menu.path.is_empty()
        })
        .map(|menu| match menu.path.as_str() {
            // 库里仍存在但前端已无对应路由的菜单路径，落到等价的静态页面，
            // 否则用户登录后会被送到一个解析不出来的地址。
            "/system/park" | "/rental/list" => "/rental/manage".to_string(),
            path => path.to_string(),
        })
        .next()
        .unwrap_or_else(|| "/".into())
}

#[cfg(test)]
mod tests {
    use spacetimedb_sdk::Timestamp;

    use super::*;

    fn role(name: &str, scope: &str) -> Role {
        Role {
            role_id: 1,
            customer_id: "public".into(),
            name: name.into(),
            remark: None,
            status: 1,
            rates: None,
            parent_id: None,
            reimbursement_auth: None,
            organization_id: None,
            scope: scope.into(),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn menu(path: &str) -> Menu {
        Menu {
            menu_id: 1,
            customer_id: "public".into(),
            name: "测试菜单".into(),
            menu_type: "menu".into(),
            status: 1,
            path: path.into(),
            active_path: None,
            redirect: None,
            component: None,
            parent_id: None,
            auth_code: None,
            template_key: None,
            template_parent_key: None,
            template_version: 1,
            template_managed: false,
            template_internal_only: false,
            template_deleted_at: None,
        }
    }

    #[test]
    fn 普通角色只能进入实时菜单授权页面() {
        let roles = [role("test", "system")];
        let menus = [menu("/")];
        assert!(can_access_route(&roles, &menus, "/"));
        assert!(!can_access_route(&roles, &menus, "/bill"));
    }

    #[test]
    fn 扫码巡检页对任何已登录账号放行() {
        // 巡检权限由权限码承载，路由层看不到权限码；没有任何菜单的账号
        // 也要能打开巡检页，无权限提示由页面与服务端给出。
        assert!(can_access_route(
            &[role("巡检员", "system")],
            &[],
            "/maintenance/inspect/transformer/3"
        ));
        // 其余维护页面仍按菜单授权，不因豁免前缀而放开。
        assert!(!can_access_route(
            &[role("巡检员", "system")],
            &[],
            "/maintenance/transformer"
        ));
    }

    #[test]
    fn 个人中心对任何已登录账号放行() {
        assert!(can_access_route(&[role("普通员工", "system")], &[], "/profile"));
    }

    #[test]
    fn 园区管理兼容原菜单路径和静态页面路径() {
        assert!(can_access_route(
            &[role("园区管理员", "tenant")],
            &[menu("/system/park")],
            "/rental/manage"
        ));
    }

    #[test]
    fn 待租厂房与侧边栏判定同一组菜单() {
        // 侧边栏按 /rental/list 决定是否显示「待租厂房」入口。路由守卫若只认
        // /rental/factory，这个角色就会看见入口、点进去却被拦下。
        let legacy = [role("招商员", "tenant")];
        let legacy_menus = [menu("/rental/list")];
        assert!(can_access_route(&legacy, &legacy_menus, "/rental/factory"));
        assert!(can_access_route(
            &legacy,
            &legacy_menus,
            "/rental/factory/detail/7"
        ));

        // 反向：只授新路径的角色同样能进，详情页跟着列表页走。
        let current = [role("招商员", "tenant")];
        let current_menus = [menu("/rental/factory")];
        assert!(can_access_route(&current, &current_menus, "/rental/factory"));
        assert!(can_access_route(
            &current,
            &current_menus,
            "/rental/factory/detail/7"
        ));

        // 两条菜单都没有的账号仍然进不去。
        assert!(!can_access_route(
            &[role("普通员工", "tenant")],
            &[menu("/bill")],
            "/rental/factory"
        ));
    }

    #[test]
    fn 设备总览对能看见设备分组任意一页的账号开放() {
        let staff = [role("物业", "tenant")];
        // 只有电表菜单的账号也能看设备总览——否则新加的这条菜单授权之前，
        // 整个设备管理分组看上去毫无变化。
        assert!(can_access_route(
            &staff,
            &[menu("/smart-meter/meter")],
            "/device/overview"
        ));
        assert!(can_access_route(
            &staff,
            &[menu("/device/access")],
            "/device/overview"
        ));

        // 摄像头页与门禁设备页各认各的菜单，不互相放行——这是两个不同的
        // 权限面，服务端 Reducer 也是按这两条路径分别校验的。
        assert!(!can_access_route(
            &staff,
            &[menu("/device/camera")],
            "/device/access"
        ));
        assert!(!can_access_route(
            &staff,
            &[menu("/device/access")],
            "/device/camera"
        ));

        // 跟设备毫无关系的账号连总览都进不去。
        assert!(!can_access_route(
            &staff,
            &[menu("/bill")],
            "/device/overview"
        ));
    }

    #[test]
    fn 园区档案接受列表和管理两侧的菜单授权() {
        // 列表页已删除，但库里的 /rental/list 菜单行还在，老角色不能因此掉权。
        assert!(can_access_route(
            &[role("园区浏览员", "tenant")],
            &[menu("/rental/list")],
            "/rental/detail/18"
        ));
        // 新入口是园区管理，只有该菜单的角色也要能打开园区档案。
        assert!(can_access_route(
            &[role("园区管理员", "tenant")],
            &[menu("/rental/manage")],
            "/rental/detail/18"
        ));
        assert!(!can_access_route(
            &[role("普通员工", "tenant")],
            &[menu("/bill")],
            "/rental/detail/18"
        ));
    }

    #[test]
    fn 系统_super_不依赖逐条菜单关系() {
        assert!(can_access_route(&[role("Super", "system")], &[], "/bill"));
    }

    #[test]
    fn 租赁总览接受该模块任意一条菜单授权() {
        let role = role("园区管理员", "tenant");
        // 只有待租厂房权限，也能看总览——它是只读汇总，不是独立业务功能。
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/rental/factory")],
            "/rental/overview"
        ));
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/rental/tenant")],
            "/rental/overview"
        ));
        // 跟租赁模块完全无关的权限不能打开总览。
        assert!(!can_access_route(&[role], &[menu("/bill")], "/rental/overview"));
    }

    #[test]
    fn 人事总览接受该分组任意一条菜单授权() {
        let role = role("人事专员", "tenant");
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/hrm/information")],
            "/hrm/overview"
        ));
        // 只有工资权限（跟员工档案没有字段关联）也能看总览——
        // 汇总页跟着侧边栏分组走，不跟着数据关系走。
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/rental/salary")],
            "/hrm/overview"
        ));
        assert!(!can_access_route(&[role], &[menu("/bill")], "/hrm/overview"));
    }

    #[test]
    fn 数据地图接受任意业务模块的菜单授权() {
        let role = role("园区经理", "tenant");
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/")],
            "/data-map"
        ));
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/maintenance/elevator")],
            "/data-map"
        ));
        // 一条业务菜单都没有的账号不能打开数据地图。
        assert!(!can_access_route(&[role], &[], "/data-map"));
    }

    #[test]
    fn 财务总览接受该分组任意一条菜单授权() {
        let role = role("财务专员", "tenant");
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/bill")],
            "/finance/overview"
        ));
        assert!(can_access_route(
            &[role.clone()],
            &[menu("/reimbursement/audit")],
            "/finance/overview"
        ));
        assert!(!can_access_route(
            &[role],
            &[menu("/hrm/information")],
            "/finance/overview"
        ));
    }
}
