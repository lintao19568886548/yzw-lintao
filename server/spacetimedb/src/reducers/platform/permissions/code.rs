//! 标准权限码管理。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id, require_menu},
        validation::{normalize_optional_text, required_text},
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn create_code(
    ctx: &ReducerContext,
    code: String,
    name: String,
    content: Option<String>,
    menu_id: Option<u64>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let code = required_text(code, "权限码不能为空")?;
    let name = required_text(name, "权限码名称不能为空")?;
    if ctx
        .db
        .code()
        .code_by_customer_value()
        .filter((customer_id.as_str(), code.as_str()))
        .any(|row| row.template_deleted_at.is_none())
    {
        return Err("权限码已经存在".into());
    }
    if let Some(menu_id) = menu_id {
        require_menu(ctx, menu_id)?;
        if ctx.db.code().iter().any(|row| {
            row.customer_id == customer_id
                && row.menu_id == Some(menu_id)
                && row.template_deleted_at.is_none()
        }) {
            return Err("菜单已经绑定权限码".into());
        }
    }
    ctx.db.code().insert(Code {
        code_id: 0,
        customer_id,
        code,
        name,
        content: normalize_optional_text(content),
        menu_id,
        created_at: ctx.timestamp,
        updated_at: None,
        template_deleted_at: None,
        template_internal_only: false,
        template_key: None,
        template_managed: false,
        template_version: 1,
    });
    Ok(())
}
