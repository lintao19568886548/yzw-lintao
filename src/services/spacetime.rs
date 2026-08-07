//! SpacetimeDB 连接建立、订阅和缓存同步。

use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
use std::{cell::Cell, collections::BTreeSet};

use dioxus::prelude::{ReadableExt, WritableExt};
use spacetimedb_sdk::{DbContext, SubscriptionHandle as _, Table};

use super::credentials::{clear_saved_token, is_durable_token, save_account, save_token};
use crate::{
    spacetime_bindings::*,
    state::{AuthState, ConnectionPhase, DataScope, ModuleLoadState, WorkspaceState},
};

thread_local! {
    /// 保留连接句柄，使页面事件可以调用生成的 Reducer 接口。
    static ACTIVE_CONNECTION: RefCell<Option<DbConnection>> = const { RefCell::new(None) };
    static AUTH_SUBSCRIPTION: RefCell<Option<SubscriptionHandle>> = const { RefCell::new(None) };
    static ACTIVE_SCOPE_SUBSCRIPTION: RefCell<Option<(DataScope, SubscriptionHandle)>> = const { RefCell::new(None) };
    static PENDING_PASSWORD_LOGIN: RefCell<Option<(String, String, bool, WorkspaceState)>> = const { RefCell::new(None) };

    /// SpacetimeDB 会在一次权限变更中连续触发大量逐行回调。浏览器端只安排一次
    /// 下一事件循环刷新，避免每插入一行就重新扫描、排序并复制整张业务表。
    #[cfg(target_arch = "wasm32")]
    static CACHE_REFRESH_SCHEDULED: Cell<bool> = const { Cell::new(false) };
    #[cfg(target_arch = "wasm32")]
    static CACHE_REFRESH_RUNNING: Cell<bool> = const { Cell::new(false) };
    #[cfg(target_arch = "wasm32")]
    static PENDING_CACHE_SCOPES: RefCell<BTreeSet<DataScope>> = const { RefCell::new(BTreeSet::new()) };
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = console, js_name = info)]
    fn yizu_realtime_info(message: &str);
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = console, js_name = error)]
    fn yizu_realtime_error(message: &str);
}

fn realtime_info(message: &str) {
    #[cfg(target_arch = "wasm32")]
    yizu_realtime_info(&format!("[YIZU-REALTIME] {message}"));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("[YIZU-REALTIME] {message}");
}

fn realtime_error(message: &str) {
    #[cfg(target_arch = "wasm32")]
    yizu_realtime_error(&format!("[YIZU-REALTIME] {message}"));

    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("[YIZU-REALTIME] ERROR: {message}");
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionConfig {
    pub server_uri: String,
    pub database_name: String,
    pub token: Option<String>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            // 公网部署时通过编译变量切换到 HTTPS/WSS 反向代理地址。
            //
            // 为兼容旧 .env，优先读取 YIZU_SPACETIMEDB_URI，
            // 兼容配置仅提供 YIZU_SPACETIMEDB_SERVER_URL 的场景。
            server_uri: option_env!("YIZU_SPACETIMEDB_URI")
                .or(option_env!("YIZU_SPACETIMEDB_SERVER_URL"))
                .unwrap_or("https://yz.furong.org")
                .into(),
            database_name: "yizu-server-yz18m".into(),
            token: super::credentials::load_saved_token(),
        }
    }
}

/// 浏览器端将同一批表更新合并到下一事件循环；原生端直接刷新。
///
/// 返回 `true` 表示当前刷新已被延后，调用方应立即返回。
fn defer_cache_refresh(state: WorkspaceState, scope: DataScope) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (state, scope);
        false
    }

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        if CACHE_REFRESH_RUNNING.with(Cell::get) {
            return false;
        }
        PENDING_CACHE_SCOPES.with(|pending| {
            pending.borrow_mut().insert(scope);
        });
        let already_scheduled = CACHE_REFRESH_SCHEDULED.with(|scheduled| {
            if scheduled.get() {
                true
            } else {
                scheduled.set(true);
                false
            }
        });
        if already_scheduled {
            return true;
        }

        let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
            CACHE_REFRESH_SCHEDULED.with(|scheduled| scheduled.set(false));
            CACHE_REFRESH_RUNNING.with(|running| running.set(true));
            ACTIVE_CONNECTION.with(|slot| {
                if let Some(connection) = slot.borrow().as_ref() {
                    let scopes = PENDING_CACHE_SCOPES
                        .with(|pending| std::mem::take(&mut *pending.borrow_mut()));
                    for scope in scopes {
                        refresh_scope(&connection.db, state, scope);
                    }
                }
            });
            CACHE_REFRESH_RUNNING.with(|running| running.set(false));
            realtime_info("合并缓存刷新完成");
        });

        let scheduled = web_sys::window().is_some_and(|window| {
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.unchecked_ref::<js_sys::Function>(),
                    0,
                )
                .is_ok()
        });
        if !scheduled {
            CACHE_REFRESH_SCHEDULED.with(|pending| pending.set(false));
            CACHE_REFRESH_RUNNING.with(|running| running.set(true));
            ACTIVE_CONNECTION.with(|slot| {
                if let Some(connection) = slot.borrow().as_ref() {
                    let scopes = PENDING_CACHE_SCOPES
                        .with(|pending| std::mem::take(&mut *pending.borrow_mut()));
                    for scope in scopes {
                        refresh_scope(&connection.db, state, scope);
                    }
                }
            });
            CACHE_REFRESH_RUNNING.with(|running| running.set(false));
        }
        true
    }
}

/// 单个 refresh 函数的统一签名——本文件所有 `refresh_x` 都长这样，
/// 才能被下面的静态表当函数指针存起来。
type RefreshFn = fn(&RemoteTables, WorkspaceState);

/// 一个 scope 被激活/被批量刷新时，应该按顺序跑哪些 refresh 函数。
///
/// 这张表只回答"跑什么"，不掺一句"怎么跑"——`refresh_scope` 才是
/// 唯一的解释器，而且只剩一个 for 循环。
///
/// 特意没有把这张表和 `scope_queries`（该订阅哪些表）合并成一份：
/// 两者看着像同一份知识的两种写法，实际不是。`DataScope::Core` 的
/// 表走的是连接建立时写死的订阅，压根不经过 `scope_queries`；
/// `DataScope::Dashboard` 和 `DataScope::Hr` 共用 `refresh_hr`，但
/// `scope_queries(Dashboard)` 故意只订阅人事表里的一张（看板只要一个
/// 异常次数，不需要完整员工数据），不是本该同步却漏掉的疏忽。硬把
/// 两张表合一，会让 Core 因为没有订阅规则而生成不出正确结果，也会让
/// Dashboard 被迫多订阅一整张不需要的人事表——这不是消掉偶然复杂度，
/// 是无视了两张表本来就承载着不同的、故意如此的知识。
fn scope_refresh_fns(scope: DataScope) -> &'static [RefreshFn] {
    match scope {
        DataScope::Core => &[
            refresh_user,
            refresh_menus,
            refresh_roles,
            refresh_permission_codes,
        ],
        DataScope::Dashboard => &[refresh_dashboard_overview, refresh_hr],
        // 数据地图要同时看到所有模块的表，刷新函数取各业务域的并集。
        // 注意：延迟批次会按每个 refresh 函数自己的域键重放，顺带把本域
        // 没订阅的次要信号（比如 my_salary_image_previews）置成空——无害，
        // 域互斥之下上一个域的数据本来就已经过期了。
        DataScope::DataMap => &[
            refresh_parks,
            refresh_tenants,
            refresh_tenant_profiles,
            refresh_rental_assets,
            refresh_salary,
            refresh_billing,
            refresh_finance,
            refresh_reimbursements,
            refresh_access_control,
            refresh_hr,
        ],
        DataScope::Permissions => &[refresh_permission_management],
        DataScope::Rental | DataScope::Maintenance => &[
            refresh_parks,
            refresh_tenants,
            refresh_tenant_profiles,
            refresh_rental_assets,
        ],
        DataScope::AccessControl => &[refresh_parks, refresh_access_control],
        DataScope::Salary => &[refresh_salary],
        DataScope::Billing => &[
            refresh_parks,
            refresh_tenants,
            refresh_billing,
            // 水电表台账与合同用表由资产刷新函数写入。
            refresh_rental_assets,
        ],
        DataScope::Finance => &[refresh_parks, refresh_finance],
        DataScope::Reimbursements => &[refresh_parks, refresh_reimbursements],
        DataScope::Hr => &[refresh_parks, refresh_hr],
        DataScope::SmartMeter => &[refresh_parks, refresh_rental_assets],
        DataScope::Device => &[refresh_parks, refresh_devices],
    }
}

fn refresh_scope(db: &RemoteTables, state: WorkspaceState, scope: DataScope) {
    for refresh in scope_refresh_fns(scope) {
        refresh(db, state);
    }
}

fn refresh_user(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Core) {
        return;
    }
    let current_user = db.current_center_user().iter().next();
    state.current_user.set(current_user.clone());
    state.auth_state.set(if current_user.is_some() {
        AuthState::SignedIn
    } else {
        AuthState::SignedOut
    });
    state.business_user.set(db.current_user().iter().next());
}

fn refresh_menus(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Core) {
        return;
    }
    let mut menus = db.my_menus().iter().collect::<Vec<_>>();
    menus.sort_by_key(|menu| (menu.parent_id.unwrap_or_default(), menu.menu_id));
    state.menus.set(menus);
}

fn refresh_roles(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Core) {
        return;
    }
    let mut roles = db.my_roles().iter().collect::<Vec<_>>();
    roles.sort_by_key(|role| role.role_id);
    state.roles.set(roles);
}

fn refresh_permission_codes(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Core) {
        return;
    }
    let mut codes = db.my_codes().iter().map(|row| row.code).collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    state.permission_codes.set(codes);
}

fn refresh_permission_management(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Permissions) {
        return;
    }
    let mut roles = db.manageable_roles().iter().collect::<Vec<_>>();
    roles.sort_by_key(|row| row.role_id);
    state.permission_roles.set(roles);

    let mut menus = db.manageable_menus().iter().collect::<Vec<_>>();
    menus.sort_by_key(|row| row.menu_id);
    state.permission_menus.set(menus);

    let mut parks = db.manageable_parks().iter().collect::<Vec<_>>();
    parks.sort_by_key(|row| row.park_id);
    state.permission_parks.set(parks);

    state
        .permission_role_menus
        .set(db.manageable_role_menus().iter().collect::<Vec<_>>());
    state
        .permission_role_parks
        .set(db.manageable_role_parks().iter().collect::<Vec<_>>());
}

fn refresh_dashboard_overview(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Dashboard) {
        return;
    }
    let overview = db.dashboard_overview().iter().next();
    if let Some(value) = overview.as_ref() {
        realtime_info(&format!(
            "总览聚合已同步 customer={} receivable={} received={} tenants={}",
            value.customer_id, value.receivable_cents, value.received_cents, value.tenant_count
        ));
    } else {
        realtime_error("总览聚合订阅已应用，但服务端没有返回 dashboard_overview 行");
    }
    state.dashboard_overview.set(overview);
}

fn refresh_salary(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Salary) {
        return;
    }
    refresh_parks(db, state);
    refresh_tenants(db, state);

    let mut salaries = db.my_salaries().iter().collect::<Vec<_>>();
    salaries.sort_by_key(|row| std::cmp::Reverse(row.salary_id));
    state.salaries.set(salaries);
    state
        .salary_image_previews
        .set(db.my_salary_image_previews().iter().collect::<Vec<_>>());
    state
        .tenant_image_previews
        .set(db.my_tenant_image_previews().iter().collect::<Vec<_>>());
}

fn refresh_rental_assets(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Rental) {
        return;
    }
    state
        .park_image_previews
        .set(db.my_park_image_previews().iter().collect::<Vec<_>>());

    let mut factories = db.my_factories().iter().collect::<Vec<_>>();
    factories.sort_by_key(|row| row.factory_id);
    state.factories.set(factories);

    let mut floors = db.my_factory_floors().iter().collect::<Vec<_>>();
    floors.sort_by_key(|row| row.floor_id);
    state.factory_floors.set(floors);

    state.factory_floor_image_previews.set(
        db.my_factory_floor_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    let mut dormitories = db.my_dormitories().iter().collect::<Vec<_>>();
    dormitories.sort_by_key(|row| row.dormitory_id);
    state.dormitories.set(dormitories);

    let mut dormitory_floors = db.my_dormitory_floors().iter().collect::<Vec<_>>();
    dormitory_floors.sort_by_key(|row| (row.dormitory_id, row.floor_no));
    state.dormitory_floors.set(dormitory_floors);

    let mut meters = db.my_utility_meters().iter().collect::<Vec<_>>();
    meters.sort_by_key(|row| row.meter_id);
    state.utility_meters.set(meters);
    state
        .dormitory_image_previews
        .set(db.my_dormitory_image_previews().iter().collect::<Vec<_>>());
    state
        .firefighting_assets
        .set(db.my_firefighting_assets().iter().collect::<Vec<_>>());
    let mut firefighting_inspections = db
        .my_firefighting_inspections()
        .iter()
        .collect::<Vec<_>>();
    firefighting_inspections
        .sort_by_key(|row| std::cmp::Reverse((row.check_time, row.inspection_id)));
    state.firefighting_inspections.set(firefighting_inspections);
    state.firefighting_asset_image_previews.set(
        db.my_firefighting_asset_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    state.firefighting_inspection_image_previews.set(
        db.my_firefighting_inspection_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    state
        .transformer_assets
        .set(db.my_transformer_assets().iter().collect::<Vec<_>>());
    let mut inspections = db.my_transformer_inspections().iter().collect::<Vec<_>>();
    // 巡检历史按时间倒序展示，最近一次在最前。
    inspections.sort_by_key(|row| std::cmp::Reverse((row.check_time, row.inspection_id)));
    state.transformer_inspections.set(inspections);
    state.transformer_asset_image_previews.set(
        db.my_transformer_asset_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    state.transformer_inspection_image_previews.set(
        db.my_transformer_inspection_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    state
        .elevator_assets
        .set(db.my_elevator_assets().iter().collect::<Vec<_>>());
    let mut elevator_inspections = db.my_elevator_inspections().iter().collect::<Vec<_>>();
    elevator_inspections.sort_by_key(|row| std::cmp::Reverse((row.check_time, row.inspection_id)));
    state.elevator_inspections.set(elevator_inspections);
    state.elevator_asset_image_previews.set(
        db.my_elevator_asset_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    state.elevator_inspection_image_previews.set(
        db.my_elevator_inspection_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
    let mut repair_orders = db.my_repair_orders().iter().collect::<Vec<_>>();
    repair_orders.sort_by_key(|row| std::cmp::Reverse(row.repair_order_id));
    state.repair_orders.set(repair_orders);
}

/// 设备管理域：设备台账，外加总览要用到的厂房与水电表台账。
///
/// 厂房与水电表两个信号也归 `refresh_rental_assets` 写，这里重复写一遍是有意的：
/// 让设备域只订自己要的五张表，而不是为了两个信号把整套租赁资产订阅拖进来。
/// 两个刷新函数不会同时跑——数据域是互斥的。
fn refresh_devices(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Device) {
        return;
    }
    let mut factories = db.my_factories().iter().collect::<Vec<_>>();
    factories.sort_by_key(|row| row.factory_id);
    state.factories.set(factories);

    let mut meters = db.my_utility_meters().iter().collect::<Vec<_>>();
    meters.sort_by_key(|row| row.meter_id);
    state.utility_meters.set(meters);

    let mut devices = db.my_device_assets().iter().collect::<Vec<_>>();
    devices.sort_by_key(|row| row.asset_id);
    state.device_assets.set(devices);

    state.device_asset_image_previews.set(
        db.my_device_asset_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );

    let mut gateways = db.my_edge_gateways().iter().collect::<Vec<_>>();
    gateways.sort_by_key(|row| row.gateway_id);
    state.edge_gateways.set(gateways);
}

fn refresh_access_control(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::AccessControl) {
        return;
    }
    let mut cars = db.my_access_cars().iter().collect::<Vec<_>>();
    cars.sort_by_key(|row| std::cmp::Reverse(row.register_time));
    state.access_cars.set(cars);

    let mut visitors = db.my_access_visitors().iter().collect::<Vec<_>>();
    visitors.sort_by_key(|row| std::cmp::Reverse(row.register_time));
    state.access_visitors.set(visitors);
}

fn refresh_billing(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Billing) {
        return;
    }
    let mut bills = db.my_amount_bills().iter().collect::<Vec<_>>();
    bills.sort_by_key(|row| std::cmp::Reverse(row.bill_id));
    state.amount_bills.set(bills);
    let mut ele_bills = db.my_ele_bills().iter().collect::<Vec<_>>();
    ele_bills.sort_by_key(|row| row.ele_id);
    state.ele_bills.set(ele_bills);
    let mut water_bills = db.my_water_bills().iter().collect::<Vec<_>>();
    water_bills.sort_by_key(|row| row.water_id);
    state.water_bills.set(water_bills);
    let mut confirmations = db
        .my_bill_collection_confirmations()
        .iter()
        .collect::<Vec<_>>();
    confirmations.sort_by_key(|row| row.confirmation_id);
    state.bill_collection_confirmations.set(confirmations);

    let mut batches = db.my_carryover_batches().iter().collect::<Vec<_>>();
    batches.sort_by_key(|row| std::cmp::Reverse(row.batch_id));
    state.carryover_batches.set(batches);
    let mut items = db.my_carryover_items().iter().collect::<Vec<_>>();
    items.sort_by_key(|row| row.item_id);
    state.carryover_items.set(items);
}

fn refresh_finance(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Finance) {
        return;
    }
    let mut rows = db.my_finances().iter().collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.transaction_time));
    state.finances.set(rows);
    state
        .finance_images
        .set(db.my_finance_images().iter().collect::<Vec<_>>());
}

fn refresh_reimbursements(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Reimbursements) {
        return;
    }
    let mut rows = db.my_reimbursements().iter().collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.id));
    state.reimbursements.set(rows);
    state.reimbursement_image_previews.set(
        db.my_reimbursement_image_previews()
            .iter()
            .collect::<Vec<_>>(),
    );
}

fn refresh_hr(db: &RemoteTables, mut state: WorkspaceState) {
    if defer_cache_refresh(state, DataScope::Hr) {
        return;
    }
    let mut employees = db.my_employees().iter().collect::<Vec<_>>();
    employees.sort_by_key(|row| std::cmp::Reverse(row.employee_id));
    state.employees.set(employees);
    state
        .employee_user_options
        .set(db.my_employee_user_options().iter().collect::<Vec<_>>());
    let mut attendances = db.my_attendances().iter().collect::<Vec<_>>();
    attendances.sort_by_key(|row| std::cmp::Reverse(row.attendance_id));
    state.attendances.set(attendances);
    let mut localizations = db.my_localizations().iter().collect::<Vec<_>>();
    localizations.sort_by_key(|row| std::cmp::Reverse(row.localization_id));
    state.localizations.set(localizations);
    let mut leaves = db.my_leave_applications().iter().collect::<Vec<_>>();
    leaves.sort_by_key(|row| std::cmp::Reverse(row.id));
    state.leave_applications.set(leaves);
    let mut locations = db.my_attendance_locations().iter().collect::<Vec<_>>();
    locations.sort_by_key(|row| row.location_id);
    state.attendance_locations.set(locations);
    state.attendance_abnormal_logs.set(
        db.my_attendance_device_abnormal_logs()
            .iter()
            .collect::<Vec<_>>(),
    );
}

fn refresh_parks(db: &RemoteTables, mut state: WorkspaceState) {
    let mut parks = db.my_parks().iter().collect::<Vec<_>>();
    parks.sort_by_key(|row| row.park_id);
    state.parks.set(parks);
}

fn refresh_tenants(db: &RemoteTables, mut state: WorkspaceState) {
    let mut tenants = db.my_rental_tenants().iter().collect::<Vec<_>>();
    tenants.sort_by_key(|row| row.rental_tenant_id);
    state.rental_tenants.set(tenants);
    state
        .rental_tenant_floors
        .set(db.my_rental_tenant_floors().iter().collect::<Vec<_>>());
    state.rental_tenant_dormitory_floors.set(
        db.my_rental_tenant_dormitory_floors()
            .iter()
            .collect::<Vec<_>>(),
    );
    state
        .rental_tenant_meters
        .set(db.my_rental_tenant_meters().iter().collect::<Vec<_>>());
    state
        .rental_tenant_fees
        .set(db.my_rental_tenant_fees().iter().collect::<Vec<_>>());
}

fn refresh_tenant_profiles(db: &RemoteTables, mut state: WorkspaceState) {
    let mut profiles = db.my_tenant_profiles().iter().collect::<Vec<_>>();
    profiles.sort_by_key(|row| row.tenant_profile_id);
    state.tenant_profiles.set(profiles);
}

fn scope_queries(scope: DataScope) -> Vec<String> {
    let queries: &[&str] = match scope {
        DataScope::Core => &[],
        DataScope::Dashboard => &["SELECT * FROM my_attendance_device_abnormal_logs"],
        // 数据地图：各业务域订阅的并集，外加三张平时走服务端分页、
        // 不做全表订阅的主表（工资、财务流水、租金账单）——地图要数
        // 全表行数和逐行核对外键归属，分页拿不到全集。只在停留于
        // /data-map 期间订阅，离开页面切换数据域时整体退订。
        DataScope::DataMap => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_factories",
            "SELECT * FROM my_factory_floors",
            "SELECT * FROM my_dormitories",
            "SELECT * FROM my_dormitory_floors",
            "SELECT * FROM my_utility_meters",
            "SELECT * FROM my_rental_tenant_meters",
            "SELECT * FROM my_rental_tenant_fees",
            "SELECT * FROM my_rental_tenants",
            "SELECT * FROM my_rental_tenant_floors",
            "SELECT * FROM my_rental_tenant_dormitory_floors",
            "SELECT * FROM my_tenant_profiles",
            "SELECT * FROM my_salaries",
            "SELECT * FROM my_amount_bills",
            "SELECT * FROM my_ele_bills",
            "SELECT * FROM my_water_bills",
            "SELECT * FROM my_finances",
            "SELECT * FROM my_reimbursements",
            "SELECT * FROM my_employees",
            "SELECT * FROM my_attendances",
            "SELECT * FROM my_leave_applications",
            "SELECT * FROM my_access_cars",
            "SELECT * FROM my_access_visitors",
            "SELECT * FROM my_firefighting_assets",
            "SELECT * FROM my_transformer_assets",
            "SELECT * FROM my_elevator_assets",
            "SELECT * FROM my_repair_orders",
        ],
        DataScope::Permissions => &[
            "SELECT * FROM manageable_roles",
            "SELECT * FROM manageable_menus",
            "SELECT * FROM manageable_parks",
            "SELECT * FROM manageable_role_menus",
            "SELECT * FROM manageable_role_parks",
        ],
        DataScope::Rental | DataScope::Maintenance => &[
            "SELECT * FROM my_rental_tenants",
            "SELECT * FROM my_rental_tenant_floors",
            "SELECT * FROM my_rental_tenant_dormitory_floors",
            "SELECT * FROM my_rental_tenant_meters",
            "SELECT * FROM my_rental_tenant_fees",
            "SELECT * FROM my_tenant_profiles",
            "SELECT * FROM my_parks",
            "SELECT * FROM my_park_image_previews",
            "SELECT * FROM my_factories",
            "SELECT * FROM my_factory_floors",
            "SELECT * FROM my_factory_floor_image_previews",
            "SELECT * FROM my_dormitories",
            "SELECT * FROM my_dormitory_floors",
            "SELECT * FROM my_dormitory_image_previews",
            "SELECT * FROM my_utility_meters",
            "SELECT * FROM my_firefighting_assets",
            "SELECT * FROM my_firefighting_asset_image_previews",
            "SELECT * FROM my_firefighting_inspections",
            "SELECT * FROM my_firefighting_inspection_image_previews",
            "SELECT * FROM my_transformer_assets",
            "SELECT * FROM my_transformer_asset_image_previews",
            "SELECT * FROM my_transformer_inspections",
            "SELECT * FROM my_transformer_inspection_image_previews",
            "SELECT * FROM my_elevator_assets",
            "SELECT * FROM my_elevator_asset_image_previews",
            "SELECT * FROM my_elevator_inspections",
            "SELECT * FROM my_elevator_inspection_image_previews",
            "SELECT * FROM my_repair_orders",
            "SELECT * FROM my_tenant_image_previews",
        ],
        DataScope::AccessControl => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_access_cars",
            "SELECT * FROM my_access_visitors",
        ],
        DataScope::Salary => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_rental_tenants",
            "SELECT * FROM my_salary_image_previews",
            "SELECT * FROM my_tenant_image_previews",
        ],
        DataScope::Billing => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_rental_tenants",
            "SELECT * FROM my_ele_bills",
            "SELECT * FROM my_water_bills",
            // 账单表单要按合同带出用表和单价、按设备号拉读数，这两张表原先
            // 漏在订阅之外——信号恒空，「按合同带入」的按钮因此从来没出现过。
            "SELECT * FROM my_utility_meters",
            "SELECT * FROM my_rental_tenant_meters",
            // 账单要按合同带入约定费用，比例和基数都在这张表上。
            "SELECT * FROM my_rental_tenant_fees",
            // 收缴确认走订阅：确认动作一落库，账单页的对账状态列实时更新，
            // 不必等下一次分页查询。账单本身仍由分页 procedure 返回。
            "SELECT * FROM my_bill_collection_confirmations",
            "SELECT * FROM my_carryover_batches",
            "SELECT * FROM my_carryover_items",
        ],
        DataScope::Finance => &["SELECT * FROM my_parks", "SELECT * FROM my_finance_images"],
        DataScope::Reimbursements => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_reimbursements",
            "SELECT * FROM my_reimbursement_image_previews",
        ],
        // 智能水电表管理：设备目录走 HTTP，这里订阅的是绑定关系要用到的自家台账
        // ——设备行要显示「已绑定到哪块表」，导入设备时要选园区和安装位置。
        DataScope::SmartMeter => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_factories",
            "SELECT * FROM my_factory_floors",
            "SELECT * FROM my_dormitories",
            "SELECT * FROM my_dormitory_floors",
            "SELECT * FROM my_utility_meters",
        ],
        // 设备管理：总览要把设备台账和水电表台账并排数一次，所以两张表都订。
        // 厂房是登记设备时选安装位置用的；楼层不订——设备位置精确到厂房加一段
        // 文字描述（「北大门东侧立杆」），再往下分层对摄像头和道闸没有意义。
        DataScope::Device => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_factories",
            "SELECT * FROM my_utility_meters",
            "SELECT * FROM my_device_assets",
            "SELECT * FROM my_device_asset_image_previews",
            "SELECT * FROM my_edge_gateways",
        ],
        DataScope::Hr => &[
            "SELECT * FROM my_parks",
            "SELECT * FROM my_employees",
            "SELECT * FROM my_employee_user_options",
            "SELECT * FROM my_attendances",
            "SELECT * FROM my_localizations",
            "SELECT * FROM my_leave_applications",
            "SELECT * FROM my_attendance_device_abnormal_logs",
            // 打卡页要在员工点按钮之前就告诉他在不在范围内，所以全员订阅，
            // 不只管理员。
            "SELECT * FROM my_attendance_locations",
        ],
    };
    queries.iter().map(|query| (*query).to_string()).collect()
}

fn set_module_state(mut state: WorkspaceState, scope: DataScope, value: ModuleLoadState) {
    state.module_states.write().insert(scope, value);
}

pub fn activate_data_scope(
    scope: Option<DataScope>,
    mut state: WorkspaceState,
) -> Result<(), String> {
    let already_active = ACTIVE_SCOPE_SUBSCRIPTION.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|(active, handle)| Some(*active) == scope && handle.is_active())
    });
    if already_active {
        return Ok(());
    }
    ACTIVE_SCOPE_SUBSCRIPTION.with(|slot| {
        if let Some((_, handle)) = slot.borrow_mut().take() {
            let _ = handle.unsubscribe();
        }
    });
    state.active_scope.set(scope);
    let Some(scope) = scope else {
        return Ok(());
    };
    let queries = scope_queries(scope);
    if queries.is_empty() {
        set_module_state(state, scope, ModuleLoadState::Ready);
        return Ok(());
    }
    set_module_state(state, scope, ModuleLoadState::Loading);
    with_connection(|connection| {
        let applied_state = state;
        let failed_state = state;
        let handle = connection
            .subscription_builder()
            .on_applied(move |ctx| {
                refresh_scope(&ctx.db, applied_state, scope);
                set_module_state(applied_state, scope, ModuleLoadState::Ready);
                realtime_info(&format!("{}按需数据已就绪", scope.label()));
            })
            .on_error(move |_, error| {
                let message = error.to_string();
                set_module_state(failed_state, scope, ModuleLoadState::Error(message.clone()));
                realtime_error(&format!("{}按需数据加载失败：{message}", scope.label()));
            })
            .subscribe(queries);
        ACTIVE_SCOPE_SUBSCRIPTION.with(|slot| *slot.borrow_mut() = Some((scope, handle)));
        Ok(())
    })
}

/// 表变更 → 状态刷新函数的声明式绑定（DSL + 解释器）。
///
/// 场景（scenario）是花括号里那张纯数据表：哪张表变了该调用哪个 refresh
/// 函数，一行一条，不掺杂"怎么订阅"的细节。解释器是宏展开本身——
/// 把每一条声明翻译成一对 `on_insert`/`on_delete` 注册，是全文件唯一
/// 知道 SpacetimeDB SDK 订阅怎么接的地方。新增一张要监听的表，只需要在
/// 表里加一行；不会再出现"表已经在刷新却漏订阅"或反过来的不同步。
///
/// 没有做成运行时可变的指令链（真正的 Free-monad 风格 DSL），是因为这里
/// 的映射在编译期就完全确定、不需要运行期组合或替换解释器（比如 mock）；
/// 宏已经能在零运行时开销的前提下给出完整的声明式表达，上运行时机制
/// （`dyn Any`/trait object 做类型擦除）只会增加复杂度，换不来任何好处。
macro_rules! wire_table_refresh {
    ($connection:expr, $state:expr, { $($table:ident => $refresh:expr),+ $(,)? }) => {
        $(
            $connection
                .db
                .$table()
                .on_insert(move |ctx, _| $refresh(&ctx.db, $state));
            $connection
                .db
                .$table()
                .on_delete(move |ctx, _| $refresh(&ctx.db, $state));
        )+
    };
}

fn register_cache_callbacks(connection: &DbConnection, state: WorkspaceState) {
    // 两张身份表的 on_delete 要清空登录态，不是"重新查一遍"这种统一模式，
    // 表达不进上面那张纯粹的映射表，所以单独手写，不硬塞进 DSL。
    let state_for_user_insert = state;
    connection
        .db
        .current_center_user()
        .on_insert(move |ctx, _| refresh_user(&ctx.db, state_for_user_insert));
    let mut state_for_user_delete = state;
    connection.db.current_center_user().on_delete(move |_, _| {
        state_for_user_delete.current_user.set(None);
        state_for_user_delete.auth_state.set(AuthState::SignedOut);
    });
    let business_user_insert_state = state;
    connection
        .db
        .current_user()
        .on_insert(move |ctx, _| refresh_user(&ctx.db, business_user_insert_state));
    let mut business_user_delete_state = state;
    connection
        .db
        .current_user()
        .on_delete(move |_, _| business_user_delete_state.business_user.set(None));

    wire_table_refresh!(connection, state, {
        my_menus => refresh_menus,
        my_roles => refresh_roles,
        // 权限码原来只在切换数据域时重读一次，管理员改完授权，已登录的人要
        // 等到重连才生效——登录页写着「实时权限同步」，这里补上那个「实时」。
        my_codes => refresh_permission_codes,
        manageable_roles => refresh_permission_management,
        manageable_menus => refresh_permission_management,
        manageable_parks => refresh_permission_management,
        manageable_role_menus => refresh_permission_management,
        manageable_role_parks => refresh_permission_management,
        dashboard_overview => refresh_dashboard_overview,
        my_salaries => refresh_salary,
        my_rental_tenants => refresh_salary,
        my_rental_tenant_floors => refresh_tenants,
        my_rental_tenant_dormitory_floors => refresh_tenants,
        my_dormitory_floors => refresh_rental_assets,
        my_utility_meters => refresh_rental_assets,
        my_rental_tenant_meters => refresh_tenants,
        my_rental_tenant_fees => refresh_tenants,
        my_attendance_locations => refresh_hr,
        my_parks => refresh_salary,
        my_salary_image_previews => refresh_salary,
        my_tenant_image_previews => refresh_salary,
        my_factories => refresh_rental_assets,
        my_tenant_profiles => refresh_rental_assets,
        my_factory_floors => refresh_rental_assets,
        my_park_image_previews => refresh_rental_assets,
        my_factory_floor_image_previews => refresh_rental_assets,
        my_dormitories => refresh_rental_assets,
        my_dormitory_image_previews => refresh_rental_assets,
        my_firefighting_assets => refresh_rental_assets,
        my_firefighting_asset_image_previews => refresh_rental_assets,
        my_firefighting_inspections => refresh_rental_assets,
        my_firefighting_inspection_image_previews => refresh_rental_assets,
        my_transformer_assets => refresh_rental_assets,
        my_transformer_asset_image_previews => refresh_rental_assets,
        my_transformer_inspections => refresh_rental_assets,
        my_transformer_inspection_image_previews => refresh_rental_assets,
        my_elevator_assets => refresh_rental_assets,
        my_elevator_asset_image_previews => refresh_rental_assets,
        my_elevator_inspections => refresh_rental_assets,
        my_elevator_inspection_image_previews => refresh_rental_assets,
        my_repair_orders => refresh_rental_assets,
        my_device_assets => refresh_devices,
        my_device_asset_image_previews => refresh_devices,
        my_edge_gateways => refresh_devices,
        my_access_cars => refresh_access_control,
        my_access_visitors => refresh_access_control,
        my_amount_bills => refresh_billing,
        my_bill_collection_confirmations => refresh_billing,
        my_carryover_batches => refresh_billing,
        my_carryover_items => refresh_billing,
        my_ele_bills => refresh_billing,
        my_water_bills => refresh_billing,
        my_finances => refresh_finance,
        my_finance_images => refresh_finance,
        my_reimbursements => refresh_reimbursements,
        my_reimbursement_image_previews => refresh_reimbursements,
        my_employees => refresh_hr,
        my_employee_user_options => refresh_hr,
        my_attendances => refresh_hr,
        my_localizations => refresh_hr,
        my_leave_applications => refresh_hr,
        my_attendance_device_abnormal_logs => refresh_hr,
    });
}

fn store_connection(connection: DbConnection) {
    ACTIVE_CONNECTION.with(|slot| {
        *slot.borrow_mut() = Some(connection);
    });
}

pub(super) fn with_connection<T>(
    action: impl FnOnce(&DbConnection) -> Result<T, String>,
) -> Result<T, String> {
    ACTIVE_CONNECTION.with(|slot| {
        let connection = slot.borrow();
        let connection = connection
            .as_ref()
            .filter(|connection| connection.is_active())
            .ok_or_else(|| "实时服务尚未连接，请稍后重试".to_string())?;
        action(connection)
    })
}

/// 使用账号密码建立业务会话；Reducer 完成后公共视图会自动推送用户和权限。
fn dispatch_password_login(
    account: String,
    password: String,
    remember_me: bool,
    mut state: WorkspaceState,
) -> Result<(), String> {
    with_connection(|connection| {
        let mut callback_state = state;
        connection
            .reducers
            .login_with_password_then(account, password, remember_me, move |_, result| {
                realtime_info(&format!("密码登录 Reducer 已返回 result={result:?}"));
                callback_state.authenticating.set(false);
                match result {
                    Ok(Ok(())) => {
                        realtime_info("密码登录验证成功");
                        callback_state.error_message.set(None);
                    }
                    Ok(Err(message)) => {
                        realtime_error(&format!("密码登录被拒绝：{message}"));
                        callback_state.auth_state.set(AuthState::SignedOut);
                        callback_state.error_message.set(Some(message));
                    }
                    Err(error) => {
                        realtime_error(&format!("密码登录请求执行失败：{error}"));
                        callback_state.auth_state.set(AuthState::SignedOut);
                        callback_state
                            .error_message
                            .set(Some(format!("登录请求执行失败：{error}")));
                    }
                }
            })
            .map(|_| realtime_info("密码登录 Reducer 已发送"))
            .map_err(|error| {
                realtime_error(&format!("无法发送登录请求：{error}"));
                format!("无法发送登录请求：{error}")
            })
    })
    .inspect_err(|_| state.authenticating.set(false))
}

pub fn login_workspace(
    account: String,
    password: String,
    remember_me: bool,
    mut state: WorkspaceState,
) -> Result<(), String> {
    realtime_info(&format!(
        "开始密码登录 account={} remember_me={remember_me}",
        account
    ));
    save_account(remember_me.then_some(account.as_str()));
    state.authenticating.set(true);
    state.auth_state.set(AuthState::SigningIn);
    state.error_message.set(None);
    match dispatch_password_login(account.clone(), password.clone(), remember_me, state) {
        Ok(()) => Ok(()),
        Err(_) => {
            PENDING_PASSWORD_LOGIN.with(|pending| {
                *pending.borrow_mut() = Some((account, password, remember_me, state))
            });
            if matches!(
                (state.phase)(),
                ConnectionPhase::Disconnected | ConnectionPhase::Failed
            ) {
                let mut callback_state = state;
                dioxus::prelude::spawn(async move {
                    if let Err(error) =
                        connect_workspace(ConnectionConfig::default(), callback_state).await
                    {
                        PENDING_PASSWORD_LOGIN.with(|pending| {
                            pending.borrow_mut().take();
                        });
                        callback_state.authenticating.set(false);
                        callback_state.auth_state.set(AuthState::SignedOut);
                        callback_state.error_message.set(Some(error));
                    }
                });
            }
            Ok(())
        }
    }
}

fn flush_pending_password_login() {
    let pending = PENDING_PASSWORD_LOGIN.with(|slot| slot.borrow_mut().take());
    if let Some((account, password, remember_me, mut state)) = pending {
        if let Err(error) = dispatch_password_login(account, password, remember_me, state) {
            state.authenticating.set(false);
            state.auth_state.set(AuthState::SignedOut);
            state.error_message.set(Some(error));
        }
    }
}

fn fail_pending_password_login(message: String) {
    let pending = PENDING_PASSWORD_LOGIN.with(|slot| slot.borrow_mut().take());
    if let Some((_, _, _, mut state)) = pending {
        state.authenticating.set(false);
        state.auth_state.set(AuthState::SignedOut);
        state.error_message.set(Some(message));
    }
}

/// 直接调用 Module Procedure 校验短信验证码并建立业务会话。
pub fn login_workspace_with_sms(
    phone_number: String,
    code: String,
    remember_me: bool,
    mut state: WorkspaceState,
) -> Result<(), String> {
    state.authenticating.set(true);
    state.error_message.set(None);
    with_connection(|connection| {
        let mut callback_state = state;
        connection.procedures.login_with_sms_code_then(
            phone_number,
            code,
            remember_me,
            move |_, result| {
                callback_state.authenticating.set(false);
                match result {
                    Ok(result) if result.success => callback_state.error_message.set(None),
                    Ok(result) => callback_state.error_message.set(Some(result.message)),
                    Err(error) => callback_state
                        .error_message
                        .set(Some(format!("短信登录 Procedure 执行失败：{error}"))),
                }
            },
        );
        Ok(())
    })
    .inspect_err(|_| state.authenticating.set(false))
}

/// 直接调用 Module Procedure 发送短信验证码。
pub fn send_login_sms_code(
    phone_number: String,
    callback: impl FnOnce(Result<u32, String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(|connection| {
        connection.procedures.send_login_sms_code_then(
            phone_number,
            move |_, result| match result {
                Ok(result) if result.success => callback(Ok(result.expires_in)),
                Ok(result) => callback(Err(result.message)),
                Err(error) => callback(Err(format!("短信发送 Procedure 执行失败：{error}"))),
            },
        );
        Ok(())
    })
}

fn reducer_result(
    result: Result<Result<(), String>, spacetimedb_sdk::error::InternalError>,
    label: &str,
) -> Result<(), String> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(message),
        Err(error) => Err(format!("{label}执行失败：{error}")),
    }
}

/// 保存报修工单；状态变更必须走独立的状态机 Reducer。
pub fn save_repair_order(
    id: Option<u64>,
    input: RepairOrderInput,
    callback: impl FnOnce(Result<(), String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(|connection| {
        let callback = move |_: &ReducerEventContext, result| {
            callback(reducer_result(result, "报修工单保存"));
        };
        let result = match id {
            Some(id) => connection
                .reducers
                .update_repair_order_then(id, input, callback),
            None => connection
                .reducers
                .create_repair_order_then(input, callback),
        };
        result.map_err(|error| format!("无法发送报修工单保存请求：{error}"))
    })
}

pub fn transition_repair_order_record(
    id: u64,
    action: String,
    remark: Option<String>,
    callback: impl FnOnce(Result<(), String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(|connection| {
        connection
            .reducers
            .transition_repair_order_then(id, action, remark, move |_, result| {
                callback(reducer_result(result, "报修工单流转"));
            })
            .map_err(|error| format!("无法发送报修工单流转请求：{error}"))
    })
}

pub fn delete_repair_order_record(
    id: u64,
    callback: impl FnOnce(Result<(), String>) + Send + 'static,
) -> Result<(), String> {
    with_connection(|connection| {
        connection
            .reducers
            .delete_repair_order_then(id, move |_, result| {
                callback(reducer_result(result, "报修工单删除"));
            })
            .map_err(|error| format!("无法发送报修工单删除请求：{error}"))
    })
}

/// 注销业务会话但保留底层设备身份，以便继续显示登录页。
pub fn logout_workspace(mut state: WorkspaceState) -> Result<(), String> {
    state.error_message.set(None);
    with_connection(|connection| {
        let mut callback_state = state;
        connection
            .reducers
            .logout_current_session_then(move |_, result| match result {
                Ok(Ok(())) => {
                    callback_state.current_user.set(None);
                    callback_state.business_user.set(None);
                    callback_state.menus.set(Vec::new());
                    callback_state.roles.set(Vec::new());
                    callback_state.dashboard_overview.set(None);
                    callback_state.rental_tenants.set(Vec::new());
                    callback_state.tenant_profiles.set(Vec::new());
                    callback_state.parks.set(Vec::new());
                    callback_state.factories.set(Vec::new());
                    callback_state.factory_floors.set(Vec::new());
                    callback_state.access_cars.set(Vec::new());
                    callback_state.access_visitors.set(Vec::new());
                    callback_state.firefighting_assets.set(Vec::new());
                    callback_state.firefighting_inspections.set(Vec::new());
                    callback_state
                        .firefighting_asset_image_previews
                        .set(Vec::new());
                    callback_state
                        .firefighting_inspection_image_previews
                        .set(Vec::new());
                    callback_state.transformer_assets.set(Vec::new());
                    callback_state.transformer_inspections.set(Vec::new());
                    callback_state
                        .transformer_asset_image_previews
                        .set(Vec::new());
                    callback_state
                        .transformer_inspection_image_previews
                        .set(Vec::new());
                    callback_state.elevator_assets.set(Vec::new());
                    callback_state.elevator_inspections.set(Vec::new());
                    callback_state
                        .elevator_asset_image_previews
                        .set(Vec::new());
                    callback_state
                        .elevator_inspection_image_previews
                        .set(Vec::new());
                    callback_state.repair_orders.set(Vec::new());
                    callback_state.salaries.set(Vec::new());
                    callback_state.salary_image_previews.set(Vec::new());
                    callback_state.tenant_image_previews.set(Vec::new());
                    callback_state.amount_bills.set(Vec::new());
                    callback_state.finances.set(Vec::new());
                    callback_state.finance_images.set(Vec::new());
                    callback_state.ele_bills.set(Vec::new());
                    callback_state.water_bills.set(Vec::new());
                    callback_state.reimbursements.set(Vec::new());
                    callback_state.reimbursement_image_previews.set(Vec::new());
                    callback_state.employees.set(Vec::new());
                    callback_state.employee_user_options.set(Vec::new());
                    callback_state.attendances.set(Vec::new());
                    callback_state.localizations.set(Vec::new());
                    callback_state.leave_applications.set(Vec::new());
                    callback_state.attendance_abnormal_logs.set(Vec::new());
                }
                Ok(Err(message)) => callback_state.error_message.set(Some(message)),
                Err(error) => callback_state
                    .error_message
                    .set(Some(format!("退出登录失败：{error}"))),
            })
            .map_err(|error| format!("无法发送退出请求：{error}"))
    })
}

fn is_rejected_saved_token(error: &str) -> bool {
    error.contains("Token verification error")
        || error.contains("401 Unauthorized")
        || error.contains("InvalidToken")
}

/// 构造一次连接；外层负责在旧设备令牌失效时决定是否重试。
async fn build_connection(
    config: ConnectionConfig,
    state: WorkspaceState,
) -> Result<DbConnection, String> {
    realtime_info(&format!(
        "开始建立连接 uri={} database={} saved_token={}",
        config.server_uri,
        config.database_name,
        config.token.is_some()
    ));
    let state_on_connect = state;
    let state_on_connect_error = state;
    let state_on_disconnect = state;
    let builder = DbConnection::builder()
        .with_uri(config.server_uri)
        .with_database_name(config.database_name)
        .with_token(config.token)
        .on_connect(move |connection, identity, token| {
            realtime_info(&format!("WebSocket 已连接 identity={identity}"));
            // 浏览器端这里回传的可能是只活 60 秒的一次性握手令牌，存下去会顶掉
            // 真正的长期令牌，详见 credentials::is_durable_token。
            if is_durable_token(token) {
                save_token(token);
            } else {
                realtime_info("本次握手令牌是短期令牌，保留原有长期令牌");
            }
            let mut callback_state = state_on_connect;
            callback_state.identity.set(Some(identity.to_string()));
            callback_state.phase.set(ConnectionPhase::Connected);
            callback_state
                .last_synced_label
                .set("实时服务已连接，权限与业务数据后台同步中".into());
            register_cache_callbacks(connection, callback_state);

            let mut applied_state = callback_state;
            let failed_state = callback_state;
            let auth_handle = connection
                .subscription_builder()
                .on_applied(move |ctx| {
                    realtime_info("认证订阅已应用");
                    refresh_user(&ctx.db, applied_state);
                    refresh_menus(&ctx.db, applied_state);
                    refresh_roles(&ctx.db, applied_state);
                    refresh_dashboard_overview(&ctx.db, applied_state);
                    applied_state
                        .last_synced_label
                        .set("账号与权限已同步".into());
                    if let Some(scope) = (applied_state.active_scope)() {
                        let _ = activate_data_scope(Some(scope), applied_state);
                    }
                })
                .on_error(move |_, error| {
                    realtime_error(&format!("后台业务数据同步失败：{error}"));
                    let mut failed_state = failed_state;
                    failed_state
                        .error_message
                        .set(Some(format!("账号与权限同步失败：{error}")));
                    // 订阅失败就再也不会回调 refresh_user，登录态会永远停在
                    // Checking。界面上「正在恢复登录状态」的占位会一直转下去，
                    // 用户连手动登录的入口都没有。这里落到已登出，把登录表单交回去。
                    if (failed_state.auth_state)() == AuthState::Checking {
                        failed_state.auth_state.set(AuthState::SignedOut);
                    }
                })
                .subscribe(vec![
                    "SELECT * FROM current_center_user".to_string(),
                    "SELECT * FROM current_user".to_string(),
                    "SELECT * FROM my_roles".to_string(),
                    "SELECT * FROM my_menus".to_string(),
                    "SELECT * FROM my_codes".to_string(),
                    "SELECT * FROM dashboard_overview".to_string(),
                ]);
            AUTH_SUBSCRIPTION.with(|slot| {
                if let Some(previous) = slot.borrow_mut().replace(auth_handle) {
                    let _ = previous.unsubscribe();
                }
            });
        })
        .on_connect_error(move |_, error| {
            realtime_error(&format!("身份连接失败：{error}"));
            let mut callback_state = state_on_connect_error;
            callback_state.phase.set(ConnectionPhase::Failed);
            callback_state
                .error_message
                .set(Some(format!("身份连接失败：{error}")));
            // 同 on_error：连不上就永远等不到登录态，必须放行到登录页。
            if (callback_state.auth_state)() == AuthState::Checking {
                callback_state.auth_state.set(AuthState::SignedOut);
            }
        })
        .on_disconnect(move |_, error| {
            realtime_error(&format!("连接已中断：{error:?}"));
            let mut callback_state = state_on_disconnect;
            callback_state.phase.set(ConnectionPhase::Disconnected);
            if let Some(error) = error {
                callback_state
                    .error_message
                    .set(Some(format!("连接已中断：{error}")));
            }
        });

    #[cfg(target_arch = "wasm32")]
    let connection = builder.build().await.map_err(|error| error.to_string())?;

    #[cfg(not(target_arch = "wasm32"))]
    let connection = builder.build().map_err(|error| error.to_string())?;

    realtime_info("连接对象构建完成");
    Ok(connection)
}

static CONNECTION_ATTEMPT_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct ConnectionAttemptGuard;

impl Drop for ConnectionAttemptGuard {
    fn drop(&mut self) {
        CONNECTION_ATTEMPT_ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub async fn connect_workspace(
    config: ConnectionConfig,
    mut state: WorkspaceState,
) -> Result<(), String> {
    if CONNECTION_ATTEMPT_ACTIVE.swap(true, std::sync::atomic::Ordering::AcqRel) {
        realtime_info("已有连接任务运行，忽略重复连接请求");
        return Ok(());
    }
    let _attempt_guard = ConnectionAttemptGuard;

    realtime_info("connect_workspace 启动");
    state.phase.set(ConnectionPhase::Connecting);
    if state.current_user.read().is_none() {
        state.auth_state.set(AuthState::Checking);
    }
    state.error_message.set(None);
    state.current_user.set(None);
    state.menus.set(Vec::new());
    state.roles.set(Vec::new());
    state.dashboard_overview.set(None);
    state.rental_tenants.set(Vec::new());
    state.tenant_profiles.set(Vec::new());
    state.parks.set(Vec::new());
    state.factories.set(Vec::new());
    state.factory_floors.set(Vec::new());
    state.access_cars.set(Vec::new());
    state.access_visitors.set(Vec::new());
    state.firefighting_assets.set(Vec::new());
    state.firefighting_inspections.set(Vec::new());
    state.firefighting_asset_image_previews.set(Vec::new());
    state.firefighting_inspection_image_previews.set(Vec::new());
    state.transformer_assets.set(Vec::new());
    state.transformer_inspections.set(Vec::new());
    state.transformer_asset_image_previews.set(Vec::new());
    state.transformer_inspection_image_previews.set(Vec::new());
    state.elevator_assets.set(Vec::new());
    state.elevator_inspections.set(Vec::new());
    state.elevator_asset_image_previews.set(Vec::new());
    state.elevator_inspection_image_previews.set(Vec::new());
    state.repair_orders.set(Vec::new());
    state.salaries.set(Vec::new());
    state.salary_image_previews.set(Vec::new());
    state.tenant_image_previews.set(Vec::new());
    state.amount_bills.set(Vec::new());
    state.finances.set(Vec::new());
    state.finance_images.set(Vec::new());
    state.ele_bills.set(Vec::new());
    state.water_bills.set(Vec::new());

    let used_saved_token = config.token.is_some();
    let connection = match build_connection(config.clone(), state).await {
        Ok(connection) => connection,
        Err(error) if used_saved_token && is_rejected_saved_token(&error) => {
            // 本地节点签发的设备令牌不能被远程节点验证，清除后匿名重连会获得新令牌。
            //
            // 这条路会换掉 identity，服务端按 identity 存的业务会话随之作废——
            // 用户看到的就是「明明勾了保持 30 天却要重新登录」。默默降级会让这类
            // 问题完全无迹可循，所以必须留下日志。
            realtime_error(&format!(
                "保存的登录令牌被拒绝，将清除并匿名重连（登录态会丢失）：{error}"
            ));
            clear_saved_token();
            state.phase.set(ConnectionPhase::Connecting);
            state.error_message.set(None);
            let retry_config = ConnectionConfig {
                token: None,
                ..config
            };
            build_connection(retry_config, state)
                .await
                .map_err(|retry_error| {
                    let message = format!("无法连接 SpacetimeDB：{retry_error}");
                    fail_pending_password_login(message.clone());
                    message
                })?
        }
        Err(error) => {
            let message = format!("无法连接 SpacetimeDB：{error}");
            fail_pending_password_login(message.clone());
            // 连接建不起来就不会有登录态回调，放行到登录页而不是卡在恢复中。
            if (state.auth_state)() == AuthState::Checking {
                state.auth_state.set(AuthState::SignedOut);
            }
            return Err(message);
        }
    };

    #[cfg(target_arch = "wasm32")]
    {
        realtime_info("启动 WASM 后台消息任务");
        connection.run_background_task();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _connection_thread = connection.run_threaded();
    }

    store_connection(connection);
    flush_pending_password_login();
    realtime_info("连接句柄已保存，实时服务进入 Connected");
    state
        .last_synced_label
        .set("实时服务已连接，正在同步账号与权限".into());
    state.phase.set(ConnectionPhase::Connected);

    Ok(())
}

#[cfg(test)]
mod scope_refresh_fns_tests {
    use super::*;

    /// 逐条核对每个 scope 该跑哪些函数——这张表本身就是数据，写错一条
    /// （漏写、多写、抄错名字）不会有编译错误提醒，只能靠测试兜底。
    #[test]
    fn 每个场景对应的刷新函数集合与预期一致() {
        // 逐条命名候选：(名字, 函数指针)。用 `std::ptr::fn_addr_eq` 比对，
        // 而不是 `==`——裸函数指针的相等性比较在新版 rustc 里已经不推荐，
        // 编译器会直接建议改用这个 API。
        const CANDIDATES: &[(&str, RefreshFn)] = &[
            ("refresh_user", refresh_user),
            ("refresh_menus", refresh_menus),
            ("refresh_roles", refresh_roles),
            ("refresh_permission_codes", refresh_permission_codes),
            ("refresh_dashboard_overview", refresh_dashboard_overview),
            ("refresh_hr", refresh_hr),
            (
                "refresh_permission_management",
                refresh_permission_management,
            ),
            ("refresh_parks", refresh_parks),
            ("refresh_tenants", refresh_tenants),
            ("refresh_tenant_profiles", refresh_tenant_profiles),
            ("refresh_rental_assets", refresh_rental_assets),
            ("refresh_devices", refresh_devices),
            ("refresh_access_control", refresh_access_control),
            ("refresh_salary", refresh_salary),
            ("refresh_billing", refresh_billing),
            ("refresh_finance", refresh_finance),
            ("refresh_reimbursements", refresh_reimbursements),
        ];

        fn names(scope: DataScope) -> Vec<&'static str> {
            scope_refresh_fns(scope)
                .iter()
                .map(|f| {
                    CANDIDATES
                        .iter()
                        .find(|(_, candidate)| std::ptr::fn_addr_eq(*f, *candidate))
                        .map_or("未知函数", |(name, _)| *name)
                })
                .collect()
        }

        assert_eq!(
            names(DataScope::Core),
            vec![
                "refresh_user",
                "refresh_menus",
                "refresh_roles",
                "refresh_permission_codes",
            ]
        );
        assert_eq!(
            names(DataScope::Dashboard),
            vec!["refresh_dashboard_overview", "refresh_hr"]
        );
        assert_eq!(
            names(DataScope::DataMap),
            vec![
                "refresh_parks",
                "refresh_tenants",
                "refresh_tenant_profiles",
                "refresh_rental_assets",
                "refresh_salary",
                "refresh_billing",
                "refresh_finance",
                "refresh_reimbursements",
                "refresh_access_control",
                "refresh_hr",
            ],
            "数据地图是各业务域刷新函数的并集"
        );
        assert_eq!(
            names(DataScope::Permissions),
            vec!["refresh_permission_management"]
        );
        assert_eq!(
            names(DataScope::Rental),
            vec![
                "refresh_parks",
                "refresh_tenants",
                "refresh_tenant_profiles",
                "refresh_rental_assets",
            ]
        );
        assert_eq!(
            names(DataScope::Maintenance),
            names(DataScope::Rental),
            "维护和租赁共用同一套刷新函数"
        );
        assert_eq!(
            names(DataScope::AccessControl),
            vec!["refresh_parks", "refresh_access_control"]
        );
        assert_eq!(names(DataScope::Salary), vec!["refresh_salary"]);
        assert_eq!(
            names(DataScope::Billing),
            vec![
                "refresh_parks",
                "refresh_tenants",
                "refresh_billing",
                "refresh_rental_assets",
            ]
        );
        assert_eq!(
            names(DataScope::Finance),
            vec!["refresh_parks", "refresh_finance"]
        );
        assert_eq!(
            names(DataScope::Reimbursements),
            vec!["refresh_parks", "refresh_reimbursements"]
        );
        assert_eq!(names(DataScope::Hr), vec!["refresh_parks", "refresh_hr"]);
        assert_eq!(
            names(DataScope::SmartMeter),
            vec!["refresh_parks", "refresh_rental_assets"]
        );
        assert_eq!(
            names(DataScope::Device),
            vec!["refresh_parks", "refresh_devices"]
        );
    }

    /// 订阅了却没接刷新回调的表 = 「保存成功」但列表纹丝不动。
    ///
    /// 这条测试是被真事故催出来的：设备台账进了 `scope_queries` 和
    /// `refresh_devices`，却漏了 `wire_table_refresh!` 里那一行。结果
    /// `create_device_asset` 成功、页面弹出「保存成功，台账正在实时更新」、
    /// 列表却要手动刷新才出现——三处都写对了，只差没人监听表变更。
    ///
    /// 宏本身只能保证「写了的表订阅和刷新一定成对」，保证不了「压根没写」。
    /// 这条测试补的正是那一步：把订阅清单和监听清单对账。
    #[test]
    fn 每张订阅的表都接了刷新回调() {
        let source = include_str!("spacetime.rs");
        let subscribed = subscribed_tables(source);
        let wired = wired_tables(source);
        assert!(!subscribed.is_empty(), "没扫到任何订阅，提取规则可能失效了");
        assert!(!wired.is_empty(), "没扫到任何监听，提取规则可能失效了");
        let missing = subscribed.difference(&wired).collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "这些表订阅了却没有刷新回调，写入会「成功但界面不更新」：{missing:?}"
        );
    }

    /// 从 `scope_queries` 的 SQL 字面量里取出被订阅的表名。
    fn subscribed_tables(source: &str) -> std::collections::BTreeSet<String> {
        let marker = concat!("SELECT * ", "FROM ");
        source
            .lines()
            .filter_map(|line| line.split_once(marker))
            .filter_map(|(_, rest)| rest.split(['"', ' ']).next())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// 从 `wire_table_refresh!` 的映射表里取出已接监听的表名。
    fn wired_tables(source: &str) -> std::collections::BTreeSet<String> {
        let mut tables = source
            .lines()
            .map(str::trim)
            .filter_map(|line| line.strip_suffix(','))
            .filter_map(|line| line.split_once(" => "))
            .filter(|(_, refresh)| refresh.starts_with("refresh_"))
            .map(|(table, _)| table.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        tables.extend(hand_wired_tables(source));
        tables
    }

    /// 两张身份表的回调是手写的（`on_delete` 要清空登录态，不是「重新查一遍」
    /// 那种统一模式），进不了映射表。这里按「`.表名().on_insert(`」的形状把它们
    /// 也认出来，而不是写死一份白名单——白名单会随着代码改动悄悄过期。
    ///
    /// 先把空白全部去掉再匹配，跨行的链式调用才不会漏。宏定义里写的是
    /// `.$table()`，不是真实表名，所以不会被误认。
    fn hand_wired_tables(source: &str) -> std::collections::BTreeSet<String> {
        let compact = source.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let mut found = std::collections::BTreeSet::new();
        for hook in [".on_insert(", ".on_delete("] {
            let mut rest = compact.as_str();
            while let Some(at) = rest.find(hook) {
                let head = &rest[..at];
                if let Some(name) = head.strip_suffix("()") {
                    let ident = name
                        .rsplit(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or_default();
                    if !ident.is_empty() {
                        found.insert(ident.to_string());
                    }
                }
                rest = &rest[at + hook.len()..];
            }
        }
        found
    }
}
