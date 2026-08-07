//! reducer 共用的身份认证与父记录校验。
//!
//! 身份解析和权限判定本身在 `crate::access`，与 View 共用同一份实现；
//! 这里只负责把判定结果翻译成 reducer 需要的 `Result<_, String>`，
//! 以及各业务表的父记录校验。

use spacetimedb::{DbContext, ReducerContext};

use crate::access::{self, Principal};
use crate::tables::{
    AmountBill, Code, Dormitory, Factory, FactoryFloor, Finance, Image, Menu, Park, RentalTenant,
    Role, Salary, SystemUser, *,
};

pub(crate) use crate::access::{ADMIN_ROLE_NAME, SYSTEM_SCOPE};

const ROLE_MANAGEMENT_PATH: &str = "/system/role";
const RENTAL_MANAGEMENT_PATHS: [&str; 2] = ["/system/park", "/rental/manage"];

/// 解析调用者；身份未绑定、会话过期或用户被禁用时给出明确原因。
fn require_principal(ctx: &ReducerContext) -> Result<Principal, String> {
    let db = ctx.db_read_only();
    let sender = ctx.sender();
    let now = Some(ctx.timestamp);
    if access::current_center_user_id(db, sender, now).is_none() {
        return Err("当前身份未绑定用户".into());
    }
    access::principal(db, sender, now).ok_or_else(|| "用户已被禁用".into())
}

/// 已经通过系统管理员校验的调用上下文（智能构造器）。
///
/// 全仓库有 150 多处 reducer 在函数体第一行手写 `require_admin(ctx)?;`，
/// 校验通过后立刻把返回的 `u64` 丢掉——校验这件事本身从未被编码进类型，
/// 完全靠"没忘记在第一行调用它"这条人工纪律撑着。这个类型把校验结果
/// 变成一个值：拿到 `AdminContext` 本身就是证明，构造入口只有
/// [`AdminContext::require`] 一个，不能绕过校验直接造一个出来。
///
/// 目前它只对外暴露"我是谁"（[`AdminContext::admin_user_id`]），还没有
/// 内部函数要求调用方传入 `&AdminContext` 才能拿到——这个仓库的 reducer
/// 目前都是自校验、自完成，没有共享的"仅管理员可调"内部辅助函数。
/// 一旦出现这样的内部函数，直接把参数类型写成 `&AdminContext`，编译器
/// 就会强制调用方已经过校验，而不必依赖review时人工检查。
pub(crate) struct AdminContext {
    // 暂时没有内部函数读取它——参见上面的类型注释。留着字段和访问器是
    // 为了不必等到第一个需要它的内部函数出现时再回来改这个类型的形状。
    #[allow(dead_code)]
    admin_user_id: u64,
}

impl AdminContext {
    /// 唯一构造入口：校验调用者是系统管理员。
    pub(crate) fn require(ctx: &ReducerContext) -> Result<Self, String> {
        require_admin(ctx).map(|admin_user_id| Self { admin_user_id })
    }

    #[allow(dead_code)]
    pub(crate) fn admin_user_id(&self) -> u64 {
        self.admin_user_id
    }
}

/// 要求调用者已经绑定启用用户，并拥有系统管理员角色。
///
/// 大多数调用方应该改用 [`AdminContext::require`]——校验结果会被编码进
/// 类型而不是一个随手丢弃的 `u64`。这个函数本身继续保留，因为部分调用点
/// 只是把它当一次性的布尔判断用（比如「本人操作或管理员」这类
/// `a || require_admin(ctx).is_err()` 的兜底分支），并不需要、也不应该
/// 构造一个新的 `AdminContext` 值。
pub(crate) fn require_admin(ctx: &ReducerContext) -> Result<u64, String> {
    let principal = require_principal(ctx)?;
    principal
        .is_admin()
        .then_some(principal.user.id)
        .ok_or("需要系统管理员权限".into())
}

/// 要求调用者拥有角色管理页面权限。
///
/// 原系统以动态菜单决定角色管理入口：系统级 `Super` 默认拥有全部菜单，
/// 其他账号只有在任一启用角色关联了角色管理菜单时才可以维护角色策略。
pub(crate) fn require_permission_manager(ctx: &ReducerContext) -> Result<u64, String> {
    let principal = require_principal(ctx)?;
    access::can_access_path(ctx.db_read_only(), &principal, ROLE_MANAGEMENT_PATH)
        .then_some(principal.user.id)
        .ok_or("需要角色管理菜单权限".into())
}

/// 要求调用者拥有原系统的园区管理菜单。
pub(crate) fn require_rental_manager(ctx: &ReducerContext) -> Result<u64, String> {
    let principal = require_principal(ctx)?;
    let allowed = principal.is_admin()
        || access::has_menu_matching(ctx.db_read_only(), &principal, |menu| {
            RENTAL_MANAGEMENT_PATHS.contains(&menu.path.as_str())
                || matches!(
                    menu.auth_code.as_deref(),
                    Some("system:park" | "rental:manage")
                )
        });
    allowed
        .then_some(principal.user.id)
        .ok_or("需要园区管理菜单权限".into())
}

/// 要求调用者拥有指定业务页面菜单；系统 Super 默认拥有全部页面。
pub(crate) fn require_menu_path(ctx: &ReducerContext, path: &str) -> Result<u64, String> {
    let principal = require_principal(ctx)?;
    access::can_access_path(ctx.db_read_only(), &principal, path)
        .then_some(principal.user.id)
        .ok_or_else(|| format!("需要 {path} 菜单权限"))
}

/// 校验园区是否落在当前账号的数据范围内。
pub(crate) fn require_park_access(ctx: &ReducerContext, park_id: u64) -> Result<(), String> {
    let principal = require_principal(ctx)?;
    access::can_access_park(ctx.db_read_only(), &principal, park_id)
        .then_some(())
        .ok_or("无权操作该园区".into())
}

/// 同 [`require_park_access`]，但园区本身已注销或不存在时放行。
///
/// 用于**注销**园区名下资产的场合。已注销的园区不在任何人的数据范围里
/// （`visible_parks` 会把它过滤掉），如果仍然要求园区权限，它名下的厂房、
/// 宿舍、水电表就永远删不掉——生产库里真的留下过这种行：园区删了，厂房还挂
/// 在台账上，谁都清不掉，因为校验的是一个谁都访问不到的园区。
///
/// 放宽的只有园区数据范围这一层：`require_factory` 等父记录校验仍然按
/// `customer_id` 隔离租户，跨租户依旧进不来。
pub(crate) fn require_park_access_unless_archived(
    ctx: &ReducerContext,
    park_id: u64,
) -> Result<(), String> {
    let archived = ctx
        .db
        .park()
        .park_id()
        .find(park_id)
        .is_none_or(|park| park.is_deleted);
    if archived {
        return Ok(());
    }
    require_park_access(ctx, park_id)
}

pub(crate) fn current_user_id(ctx: &ReducerContext) -> Option<u64> {
    access::current_user_id(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
}

pub(crate) fn current_center_user_id(ctx: &ReducerContext) -> Option<u64> {
    access::current_center_user_id(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
}

pub(crate) fn current_customer_id(ctx: &ReducerContext) -> Option<String> {
    access::current_customer_id(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
}

pub(crate) fn require_user(ctx: &ReducerContext, user_id: u64) -> Result<SystemUser, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .system_user()
        .id()
        .find(user_id)
        .filter(|user| user.customer_id == customer_id)
        .ok_or("用户不存在".into())
}

pub(crate) fn require_role(ctx: &ReducerContext, role_id: u64) -> Result<Role, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .role()
        .role_id()
        .find(role_id)
        .filter(|role| role.customer_id == customer_id)
        .ok_or("角色不存在".into())
}

pub(crate) fn require_menu(ctx: &ReducerContext, menu_id: u64) -> Result<Menu, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .menu()
        .menu_id()
        .find(menu_id)
        .filter(|menu| menu.customer_id == customer_id)
        .ok_or("菜单不存在".into())
}

pub(crate) fn require_code(ctx: &ReducerContext, code_id: u64) -> Result<Code, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .code()
        .code_id()
        .find(code_id)
        .filter(|code| code.customer_id == customer_id && code.template_deleted_at.is_none())
        .ok_or("权限码不存在".into())
}

pub(crate) fn require_park(ctx: &ReducerContext, park_id: u64) -> Result<Park, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .park()
        .park_id()
        .find(park_id)
        .filter(|park| park.customer_id == customer_id && !park.is_deleted)
        .ok_or("园区不存在".into())
}

pub(crate) fn require_factory(ctx: &ReducerContext, factory_id: u64) -> Result<Factory, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .factory()
        .factory_id()
        .find(factory_id)
        .filter(|factory| factory.customer_id == customer_id && !factory.is_deleted)
        .ok_or("厂房不存在".into())
}

pub(crate) fn require_factory_floor(
    ctx: &ReducerContext,
    floor_id: u64,
) -> Result<FactoryFloor, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .factory_floor()
        .floor_id()
        .find(floor_id)
        .filter(|floor| floor.customer_id == customer_id && !floor.is_deleted)
        .ok_or("厂房楼层不存在".into())
}

pub(crate) fn require_dormitory_floor(
    ctx: &ReducerContext,
    dormitory_floor_id: u64,
) -> Result<DormitoryFloor, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .dormitory_floor()
        .dormitory_floor_id()
        .find(dormitory_floor_id)
        .filter(|floor| floor.customer_id == customer_id && !floor.is_deleted)
        .ok_or("宿舍楼层不存在".into())
}

pub(crate) fn require_image(ctx: &ReducerContext, img_id: u64) -> Result<Image, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .image()
        .img_id()
        .find(img_id)
        .filter(|image| image.customer_id == customer_id)
        .ok_or("图片不存在".into())
}

pub(crate) fn require_dormitory(
    ctx: &ReducerContext,
    dormitory_id: u64,
) -> Result<Dormitory, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .dormitory_building()
        .dormitory_id()
        .find(dormitory_id)
        .filter(|dormitory| dormitory.customer_id == customer_id && !dormitory.is_deleted)
        .ok_or("宿舍不存在".into())
}

pub(crate) fn require_rental_tenant(
    ctx: &ReducerContext,
    rental_tenant_id: u64,
) -> Result<RentalTenant, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .rental_tenant()
        .rental_tenant_id()
        .find(rental_tenant_id)
        .filter(|tenant| tenant.customer_id == customer_id && !tenant.is_deleted)
        .ok_or("租赁客户不存在".into())
}

pub(crate) fn require_salary(ctx: &ReducerContext, salary_id: u64) -> Result<Salary, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .employee_salary()
        .salary_id()
        .find(salary_id)
        .filter(|salary| salary.customer_id == customer_id && !salary.is_deleted)
        .ok_or("工资记录不存在".into())
}

pub(crate) fn require_finance(ctx: &ReducerContext, finance_id: u64) -> Result<Finance, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .finance()
        .finance_id()
        .find(finance_id)
        .filter(|finance| finance.customer_id == customer_id && !finance.is_deleted)
        .ok_or("财务流水不存在".into())
}

pub(crate) fn require_amount_bill(
    ctx: &ReducerContext,
    bill_id: u64,
) -> Result<AmountBill, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .amount_bill()
        .bill_id()
        .find(bill_id)
        .filter(|bill| bill.customer_id == customer_id)
        .ok_or("应收账单不存在".into())
}

pub(crate) fn require_center_user(
    ctx: &ReducerContext,
    center_user_id: u64,
) -> Result<CenterUser, String> {
    ctx.db
        .center_user()
        .id()
        .find(center_user_id)
        .ok_or("中心用户不存在".into())
}

pub(crate) fn require_customer(
    ctx: &ReducerContext,
    customer_id: &String,
) -> Result<Customer, String> {
    ctx.db
        .customer()
        .customer_id()
        .find(customer_id)
        .filter(|customer| customer.status == 1)
        .ok_or("租户不存在或已停用".into())
}

pub(crate) fn require_organization(
    ctx: &ReducerContext,
    organization_id: u64,
) -> Result<Organization, String> {
    ctx.db
        .organization()
        .id()
        .find(organization_id)
        .filter(|organization| organization.status == "active")
        .ok_or("组织不存在或已停用".into())
}
