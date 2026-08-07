//! Reducer 与 View 共用的身份解析和权限判定。
//!
//! 这里是权限语义的唯一事实来源。Reducer 和 View 的上下文类型不同，但
//! `DbContext::db_read_only()` 对两者都返回同一个 `LocalReadOnly`，因此判定
//! 逻辑不需要写两遍——写两遍就会像历史实现那样悄悄长出行为差异
//! （View 侧曾经漏掉禁用用户和菜单租户归属校验）。
//!
//! 调用方只负责把 `ctx.db_read_only()`、`ctx.sender()` 和可选的当前时间传进来。

use std::collections::BTreeSet;

use spacetimedb::{Identity, LocalReadOnly, Timestamp};

use crate::{reducers::shared::park_ref::NO_PARK, tables::*};

pub const ADMIN_ROLE_NAME: &str = "Super";
pub const SYSTEM_SCOPE: &str = "system";

/// 已解析的调用者：启用状态的业务用户，以及它的全部启用角色。
///
/// 能构造出 `Principal` 就意味着「身份已绑定、会话有效、用户未被禁用、
/// 用户属于当前租户」都已成立，后续判定只需要在此基础上加条件。
pub struct Principal {
    pub user: SystemUser,
    pub roles: Vec<Role>,
}

impl Principal {
    /// 系统级 `Super` 拥有全部菜单和全部园区。
    pub fn is_admin(&self) -> bool {
        self.roles
            .iter()
            .any(|role| role.name == ADMIN_ROLE_NAME && role.scope == SYSTEM_SCOPE)
    }
}

/// 解析当前身份对应的中心账号。
///
/// `now` 为 `Some` 时（Reducer）会拒绝已过期的会话。View 拿不到时间源，只能
/// 传 `None`，转而依赖 `sweep_expired_sessions` 保证过期会话行已被删除。
pub fn current_center_user_id(
    db: &LocalReadOnly,
    sender: Identity,
    now: Option<Timestamp>,
) -> Option<u64> {
    db.user_session()
        .identity()
        .find(sender)
        .filter(|session| now.is_none_or(|now| session.expires_at > now))
        .map(|session| session.center_user_id)
        .or_else(|| {
            // 管理员显式绑定的常驻设备身份，不依赖登录会话。
            db.user_identity()
                .identity()
                .find(sender)
                .map(|binding| binding.center_user_id)
        })
}

pub fn current_customer_id(
    db: &LocalReadOnly,
    sender: Identity,
    now: Option<Timestamp>,
) -> Option<String> {
    let center_user_id = current_center_user_id(db, sender, now)?;
    db.center_user().id().find(center_user_id)?.customer_type
}

pub fn current_user_id(
    db: &LocalReadOnly,
    sender: Identity,
    now: Option<Timestamp>,
) -> Option<u64> {
    let center_user_id = current_center_user_id(db, sender, now)?;
    let customer_id = current_customer_id(db, sender, now)?;
    db.user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user_id, customer_id.as_str()))
        .next()
        .map(|mapping| mapping.customer_user_id)
}

/// 解析调用者，并在用户被禁用或不属于当前租户时返回 `None`。
pub fn principal(
    db: &LocalReadOnly,
    sender: Identity,
    now: Option<Timestamp>,
) -> Option<Principal> {
    let customer_id = current_customer_id(db, sender, now)?;
    let user_id = current_user_id(db, sender, now)?;
    let user = db
        .system_user()
        .id()
        .find(user_id)
        .filter(|user| user.customer_id == customer_id && user.status == 1)?;
    let mut roles = db
        .user_role()
        .user_role_by_user()
        .filter(user_id)
        .filter_map(|link| db.role().role_id().find(link.role_id))
        .filter(|role| role.status == 1)
        .collect::<Vec<_>>();
    roles.sort_by_key(|role| role.role_id);
    roles.dedup_by_key(|role| role.role_id);
    Some(Principal { user, roles })
}

/// 调用者是否通过任一启用角色拿到满足条件的有效菜单。
///
/// 租户归属、启用状态和模板删除标记在这里统一校验，谓词只描述业务条件。
pub fn has_menu_matching(
    db: &LocalReadOnly,
    principal: &Principal,
    predicate: impl Fn(&Menu) -> bool,
) -> bool {
    principal.roles.iter().any(|role| {
        db.role_menu()
            .role_menu_by_role()
            .filter(role.role_id)
            .filter(|link| !link.is_deleted)
            .filter_map(|link| db.menu().menu_id().find(link.menu_id))
            .any(|menu| {
                menu.customer_id == principal.user.customer_id
                    && menu.status == 1
                    && menu.template_deleted_at.is_none()
                    && predicate(&menu)
            })
    })
}

/// 调用者是否有权访问指定业务页面；系统管理员默认拥有全部页面。
pub fn can_access_path(db: &LocalReadOnly, principal: &Principal, path: &str) -> bool {
    principal.is_admin() || has_menu_matching(db, principal, |menu| menu.path == path)
}

/// 调用者数据范围内的园区。
///
/// 与原系统一致：用户直接授权优先，只有完全没有直接授权时才回退到角色园区。
pub fn visible_parks(db: &LocalReadOnly, principal: &Principal) -> Vec<Park> {
    if principal.is_admin() {
        let mut parks = db
            .park()
            .park_by_customer()
            .filter(principal.user.customer_id.as_str())
            .filter(|park| !park.is_deleted)
            .collect::<Vec<_>>();
        parks.sort_by_key(|park| park.park_id);
        return parks;
    }

    let mut parks = db
        .user_park()
        .user_park_by_user()
        .filter(principal.user.id)
        .filter(|link| !link.is_deleted)
        .filter_map(|link| db.park().park_id().find(link.park_id))
        .filter(|park| !park.is_deleted)
        .collect::<Vec<_>>();
    if parks.is_empty() {
        for role in &principal.roles {
            parks.extend(
                db.role_park()
                    .role_park_by_role()
                    .filter(role.role_id)
                    .filter(|link| !link.is_deleted)
                    .filter_map(|link| db.park().park_id().find(link.park_id))
                    .filter(|park| !park.is_deleted),
            );
        }
    }
    parks.sort_by_key(|park| park.park_id);
    parks.dedup_by_key(|park| park.park_id);
    parks
}

pub fn can_access_park(db: &LocalReadOnly, principal: &Principal, park_id: u64) -> bool {
    visible_parks(db, principal)
        .iter()
        .any(|park| park.park_id == park_id)
}

/// 人事全员数据权限码。持有它的角色可以看到全公司的员工、考勤和请假数据。
pub const CODE_HR_MANAGE: &str = "hr:manage";

/// 设施巡检权限码。持有它的账号可以扫码或在后台登记巡检记录（变压器等）。
///
/// 用权限码而不是菜单承载：巡检员不需要看到后台维护页；与台账管理权限分离，
/// 能填报巡检的人未必能增删设备。见 `docs/变压器台账与扫码巡检.md` §3.4。
pub const CODE_MAINTENANCE_INSPECT: &str = "maintenance:inspect";

/// 角色的职能：可以越过园区或行主限制多看到一部分数据的能力。
///
/// 职能必须由**显式授予**决定——权限码关联或角色表上的显式字段——绝不能靠角色名
/// 字符串判断。靠名字判断意味着在权限管理页给角色改个名，就会静默改变一批人的
/// 可见范围，既不报错也无痕迹。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Duty {
    /// 审批报销：可以看到本人以外、数据范围内的报销申请。
    ///
    /// 由 `Role.reimbursement_auth` 承载，因为它同时是审批额度而不只是开关。
    ReimbursementAudit,
    /// 人事全员数据：可以看到本人以外的员工、考勤和请假记录。
    HrManage,
}

/// 一次读取授权判定所需的全部上下文。
///
/// 这是读取权限模型的类型化表达：**租户 × 园区 × 行主**，职能标志可以放宽后两层。
/// View 只解析一次 `ReadScope`，随后逐行判定都走它的方法，不再手写过滤条件——
/// 手写三层过滤时漏掉任何一层都是一次越权读取，而漏写不会有任何编译错误。
pub struct ReadScope {
    pub customer_id: String,
    /// 租户业务用户 id（`SystemUser.id`）。
    ///
    /// 注意它与中心账号 `center_user_id` 是**两套独立主键**，由
    /// `user_tenant_mapping` 相互映射。`center/*` 下的数据以 center 侧 id 标记
    /// 行主，不能用这个字段判定，否则就是把两套 id 混为一谈。
    pub user_id: u64,
    /// 调用者数据范围内的园区。系统管理员在构造时就展开为本租户全部园区，
    /// 使 `parks()` 对两类账号都能直接迭代。
    park_ids: BTreeSet<u64>,
    /// 是否不受园区限制（系统管理员）。决定未归属园区的记录是否可见。
    unrestricted: bool,
    roles: Vec<Role>,
    /// 调用者持有的权限码：角色继承的加上直接授予本人的。
    codes: BTreeSet<String>,
}

impl ReadScope {
    /// 是否不受园区限制（系统管理员）。
    ///
    /// 用于本身没有园区维度、只区分「管理员看全部 / 其他人看自己」的数据。
    pub fn is_unrestricted(&self) -> bool {
        self.unrestricted
    }

    /// 园区是否在数据范围内。
    pub fn allows_park(&self, park_id: u64) -> bool {
        self.unrestricted || self.park_ids.contains(&park_id)
    }

    /// 园区可空的记录：未归属园区（[`NO_PARK`]）的记录只有不受园区限制时才可见。
    ///
    /// 园区外键统一用 0 表示「没有园区」，原因见 `reducers::shared::park_ref`。
    pub fn allows_optional_park(&self, park_id: u64) -> bool {
        if park_id == NO_PARK {
            return self.unrestricted;
        }
        self.allows_park(park_id)
    }

    /// 数据范围内的园区 id。供按园区索引逐个取行的 view 使用。
    pub fn parks(&self) -> impl Iterator<Item = u64> + '_ {
        self.park_ids.iter().copied()
    }

    /// 记录是否属于调用者本人。
    pub fn owns(&self, owner: Option<u64>) -> bool {
        owner == Some(self.user_id)
    }

    /// 行主字段非空时的本人判定。
    pub fn owns_id(&self, owner: u64) -> bool {
        owner == self.user_id
    }

    /// 调用者是否持有指定权限码。
    pub fn has_code(&self, code: &str) -> bool {
        self.codes.contains(code)
    }

    /// 调用者是否具备某项职能。
    pub fn has_duty(&self, duty: Duty) -> bool {
        if self.unrestricted {
            // 系统管理员具备全部职能。
            return true;
        }
        match duty {
            Duty::ReimbursementAudit => self
                .roles
                .iter()
                .any(|role| role.reimbursement_auth.unwrap_or(0) > 0),
            Duty::HrManage => self.has_code(CODE_HR_MANAGE),
        }
    }

    /// 本人的记录始终可见；具备职能时还能看到数据范围内其他人的记录。
    pub fn owns_or_has_duty(&self, owner: Option<u64>, park_id: u64, duty: Duty) -> bool {
        self.owns(owner) || (self.has_duty(duty) && self.allows_optional_park(park_id))
    }
}

#[cfg(test)]
impl ReadScope {
    /// 构造受园区限制的读取范围，仅供判定规则的单元测试使用。
    fn scoped(user_id: u64, park_ids: impl IntoIterator<Item = u64>, roles: Vec<Role>) -> Self {
        Self {
            customer_id: "public".into(),
            user_id,
            park_ids: park_ids.into_iter().collect(),
            unrestricted: false,
            roles,
            codes: BTreeSet::new(),
        }
    }

    /// 构造持有指定权限码的读取范围。
    fn with_codes(user_id: u64, codes: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            customer_id: "public".into(),
            user_id,
            park_ids: BTreeSet::new(),
            unrestricted: false,
            roles: Vec::new(),
            codes: codes.into_iter().map(str::to_string).collect(),
        }
    }

    /// 构造不受园区限制的读取范围（系统管理员）。
    fn unrestricted(user_id: u64, park_ids: impl IntoIterator<Item = u64>) -> Self {
        Self {
            customer_id: "public".into(),
            user_id,
            park_ids: park_ids.into_iter().collect(),
            unrestricted: true,
            roles: Vec::new(),
            codes: BTreeSet::new(),
        }
    }
}

/// 解析调用者的读取范围；身份无效、用户被禁用或不属于任何租户时返回 `None`。
pub fn read_scope(
    db: &LocalReadOnly,
    sender: Identity,
    now: Option<Timestamp>,
) -> Option<ReadScope> {
    let principal = principal(db, sender, now)?;
    let unrestricted = principal.is_admin();
    // 管理员的园区集合同样展开为本租户全部园区：既保持 `allows_park` 的语义，
    // 又让按园区索引取行的 view 不必区分两类账号。
    let park_ids = visible_parks(db, &principal)
        .into_iter()
        .map(|park| park.park_id)
        .collect::<BTreeSet<_>>();
    let codes = held_codes(db, &principal);
    Some(ReadScope {
        customer_id: principal.user.customer_id,
        user_id: principal.user.id,
        park_ids,
        unrestricted,
        roles: principal.roles,
        codes,
    })
}

/// 调用者持有的权限码：角色继承的加上直接授予本人的。
///
/// 与 `views::permissions::my_codes` 的口径保持一致，只是这里不需要区分来源。
fn held_codes(db: &LocalReadOnly, principal: &Principal) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    for role in &principal.roles {
        for link in db.role_code().role_code_by_role().filter(role.role_id) {
            if let Some(code) = db.code().code_id().find(link.code_id)
                && code.template_deleted_at.is_none()
            {
                codes.insert(code.code);
            }
        }
    }
    for direct in db.user_code().user_code_by_user().filter(principal.user.id) {
        codes.insert(direct.code);
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(reimbursement_auth: Option<i32>) -> Role {
        Role {
            role_id: 1,
            customer_id: "public".into(),
            name: "财务".into(),
            remark: None,
            status: 1,
            rates: None,
            parent_id: None,
            reimbursement_auth,
            organization_id: None,
            scope: "tenant".into(),
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 园区范围外的记录不可读() {
        let scope = ReadScope::scoped(7, [1, 2], vec![role(None)]);
        assert!(scope.allows_park(1));
        assert!(!scope.allows_park(3));
    }

    #[test]
    fn 系统管理员不受园区限制() {
        let scope = ReadScope::unrestricted(1, [1, 2]);
        assert!(scope.is_unrestricted());
        assert!(scope.allows_park(999));
        assert!(scope.allows_optional_park(NO_PARK));
    }

    #[test]
    fn 园区集合对两类账号都可迭代() {
        let limited = ReadScope::scoped(7, [3, 1], vec![role(None)]);
        assert_eq!(limited.parks().collect::<Vec<_>>(), vec![1, 3]);
        // 管理员的园区集合在构造时已展开为本租户全部园区。
        let admin = ReadScope::unrestricted(1, [1, 2]);
        assert_eq!(admin.parks().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn 非空行主字段的本人判定() {
        let scope = ReadScope::scoped(7, [1], vec![role(None)]);
        assert!(scope.owns_id(7));
        assert!(!scope.owns_id(8));
    }

    #[test]
    fn 未归属园区的记录对受限账号不可读() {
        let scope = ReadScope::scoped(7, [1], vec![role(None)]);
        assert!(!scope.is_unrestricted());
        assert!(!scope.allows_optional_park(NO_PARK));
    }

    #[test]
    fn 本人记录始终可读且不依赖园区() {
        let scope = ReadScope::scoped(7, [1], vec![role(None)]);
        assert!(scope.owns(Some(7)));
        assert!(!scope.owns(Some(8)));
        assert!(!scope.owns(None));
        // 记录落在园区范围外，但属于本人，仍然可读。
        assert!(scope.owns_or_has_duty(Some(7), 99, Duty::ReimbursementAudit));
    }

    #[test]
    fn 职能只放宽数据范围内的他人记录() {
        let auditor = ReadScope::scoped(7, [1], vec![role(Some(1))]);
        assert!(auditor.has_duty(Duty::ReimbursementAudit));
        assert!(auditor.owns_or_has_duty(Some(8), 1, Duty::ReimbursementAudit));
        // 有职能也看不到数据范围之外的园区。
        assert!(!auditor.owns_or_has_duty(Some(8), 2, Duty::ReimbursementAudit));

        let plain = ReadScope::scoped(7, [1], vec![role(None)]);
        assert!(!plain.has_duty(Duty::ReimbursementAudit));
        assert!(!plain.owns_or_has_duty(Some(8), 1, Duty::ReimbursementAudit));
    }

    #[test]
    fn 人事职能由权限码授予而非角色名() {
        // 角色叫什么都不影响可见范围，只看有没有拿到权限码。
        let mut hr_role = role(None);
        hr_role.name = "人事部".into();
        let by_name_only = ReadScope::scoped(7, [1], vec![hr_role]);
        assert!(!by_name_only.has_duty(Duty::HrManage));

        let granted = ReadScope::with_codes(7, [CODE_HR_MANAGE]);
        assert!(granted.has_duty(Duty::HrManage));
        assert!(granted.has_code(CODE_HR_MANAGE));

        // 持有别的权限码不会顺带获得人事职能。
        let unrelated = ReadScope::with_codes(7, ["billing:export"]);
        assert!(!unrelated.has_duty(Duty::HrManage));
    }

    #[test]
    fn 系统管理员具备全部职能() {
        let scope = ReadScope::unrestricted(1, [1]);
        assert!(scope.has_duty(Duty::ReimbursementAudit));
        assert!(scope.has_duty(Duty::HrManage));
    }
}
