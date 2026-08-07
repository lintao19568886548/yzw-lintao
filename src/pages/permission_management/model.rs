//! 权限管理页面的纯数据模型与层级计算。

use std::collections::{BTreeMap, BTreeSet};

use crate::spacetime_bindings::{
    menu_type::Menu, role_menu_type::RoleMenu, role_park_type::RolePark, role_type::Role,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct RoleDraft {
    pub role_id: Option<u64>,
    pub name: String,
    pub remark: String,
    pub status: i8,
    pub parent_id: Option<u64>,
    pub reimbursement_auth: Option<i32>,
    pub rates: String,
    pub menu_ids: BTreeSet<u64>,
    pub park_ids: BTreeSet<u64>,
}

impl RoleDraft {
    #[pure_function::pure]
    pub fn new(parent_id: Option<u64>) -> Self {
        Self {
            status: 1,
            parent_id,
            ..Self::default()
        }
    }

    #[pure_function::pure]
    pub fn from_role(role: &Role, menu_links: &[RoleMenu], park_links: &[RolePark]) -> Self {
        Self {
            role_id: Some(role.role_id),
            name: role.name.clone(),
            remark: role.remark.clone().unwrap_or_default(),
            status: role.status,
            parent_id: role.parent_id,
            reimbursement_auth: role.reimbursement_auth,
            // 兼容迁移前的历史角色：拒绝审核时统一按零额度展示。
            rates: if role.reimbursement_auth == Some(0) {
                "0".into()
            } else {
                role.rates
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            },
            menu_ids: menu_links
                .iter()
                .filter(|link| link.role_id == role.role_id && !link.is_deleted)
                .map(|link| link.menu_id)
                .collect(),
            park_ids: park_links
                .iter()
                .filter(|link| link.role_id == role.role_id && !link.is_deleted)
                .map(|link| link.park_id)
                .collect(),
        }
    }
}

#[pure_function::pure]
pub(super) fn ordered_roles(roles: &[Role]) -> Vec<(Role, usize)> {
    let ids = roles
        .iter()
        .map(|role| role.role_id)
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<Option<u64>, Vec<Role>>::new();
    for role in roles {
        let parent = role.parent_id.filter(|parent_id| ids.contains(parent_id));
        children.entry(parent).or_default().push(role.clone());
    }
    for rows in children.values_mut() {
        rows.sort_by_key(|role| role.role_id);
    }

    fn append(
        parent_id: Option<u64>,
        depth: usize,
        children: &BTreeMap<Option<u64>, Vec<Role>>,
        visited: &mut BTreeSet<u64>,
        output: &mut Vec<(Role, usize)>,
    ) {
        for role in children.get(&parent_id).into_iter().flatten() {
            if !visited.insert(role.role_id) {
                continue;
            }
            output.push((role.clone(), depth));
            append(Some(role.role_id), depth + 1, children, visited, output);
        }
    }

    let mut output = Vec::new();
    append(None, 0, &children, &mut BTreeSet::new(), &mut output);
    output
}

#[pure_function::pure]
pub(super) fn menu_depth(menu: &Menu, menus: &[Menu]) -> usize {
    let parents = menus
        .iter()
        .map(|item| (item.menu_id, item.parent_id))
        .collect::<BTreeMap<_, _>>();
    let mut depth = 0usize;
    let mut parent_id = menu.parent_id;
    let mut visited = BTreeSet::new();
    while let Some(id) = parent_id {
        if !visited.insert(id) || depth >= 5 {
            break;
        }
        depth += 1;
        parent_id = parents.get(&id).copied().flatten();
    }
    depth
}

#[pure_function::pure]
pub(super) fn allowed_menu_ids(
    draft: &RoleDraft,
    roles: &[Role],
    menus: &[Menu],
    role_menus: &[RoleMenu],
) -> BTreeSet<u64> {
    let Some(parent_id) = draft.parent_id else {
        return menus.iter().map(|menu| menu.menu_id).collect();
    };
    let parent = roles.iter().find(|role| role.role_id == parent_id);
    let inherited = role_menus
        .iter()
        .filter(|link| link.role_id == parent_id && !link.is_deleted)
        .map(|link| link.menu_id)
        .collect::<BTreeSet<_>>();
    if inherited.is_empty()
        && parent.is_some_and(|role| role.name == "Super" && role.scope == "system")
    {
        menus.iter().map(|menu| menu.menu_id).collect()
    } else {
        inherited
    }
}

#[pure_function::pure]
pub(super) fn role_is_descendant(roles: &[Role], candidate_id: u64, role_id: u64) -> bool {
    let map = roles
        .iter()
        .map(|role| (role.role_id, role.parent_id))
        .collect::<BTreeMap<_, _>>();
    let mut next = Some(candidate_id);
    let mut visited = BTreeSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return false;
        }
        if id == role_id {
            return true;
        }
        next = map.get(&id).copied().flatten();
    }
    false
}

#[pure_function::pure]
pub(super) fn role_scope_label(role: &Role) -> &'static str {
    match role.scope.as_str() {
        "organization" => "组织范围",
        "system" => "系统范围",
        _ => "租户范围",
    }
}
