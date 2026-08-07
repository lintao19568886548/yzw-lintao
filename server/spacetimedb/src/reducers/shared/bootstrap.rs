//! 首次发布后的管理员初始化逻辑。

use spacetimedb::{ReducerContext, Table};

use super::{
    access::{ADMIN_ROLE_NAME, SYSTEM_SCOPE},
    validation::required_text,
};
use crate::reducers::platform::{
    center::system::app_version::insert_default_center_app_version,
    system::app_version::insert_default_app_version,
};
use crate::tables::*;

/// 仅允许在空数据库执行一次，并把调用者身份绑定为系统管理员。
#[spacetimedb::reducer]
pub fn bootstrap_admin(
    ctx: &ReducerContext,
    username: String,
    real_name: String,
) -> Result<(), String> {
    if ctx.db.center_user().count() != 0 || ctx.db.user_identity().count() != 0 {
        return Err("系统已经完成初始化".into());
    }
    let username = required_text(username, "账号不能为空")?;
    let real_name = required_text(real_name, "姓名不能为空")?;
    let customer_id = "public".to_string();
    ctx.db.customer().insert(Customer {
        customer_id: customer_id.clone(),
        name: "公共空间".into(),
        city: None,
        company_short_name: None,
        code: Some("public".into()),
        status: 1,
        db_name: Some("public_magic".into()),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    let center_user = ctx.db.center_user().insert(CenterUser {
        id: 0,
        username: username.clone(),
        real_name: real_name.clone(),
        customer_type: Some(customer_id.clone()),
        status: 1,
        token_version: 1,
        phone: None,
        home_path: Some("/analytics".into()),
        membership_trial_start_at: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    let role = ctx.db.role().insert(Role {
        role_id: 0,
        customer_id: customer_id.clone(),
        name: ADMIN_ROLE_NAME.into(),
        remark: Some("系统初始化管理员".into()),
        status: 1,
        rates: None,
        parent_id: None,
        reimbursement_auth: None,
        organization_id: None,
        scope: SYSTEM_SCOPE.into(),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    let user = ctx.db.system_user().insert(SystemUser {
        id: 0,
        username,
        customer_id: customer_id.clone(),
        real_name,
        home_path: Some("/analytics".into()),
        phone: None,
        customer_type: None,
        status: 1,
        token_version: 1,
        created_at: ctx.timestamp,
        updated_at: None,
        avatar_url: None,
    });
    ctx.db.user_identity().insert(UserIdentity {
        identity: ctx.sender(),
        center_user_id: center_user.id,
        created_at: ctx.timestamp,
    });
    ctx.db.user_tenant_mapping().insert(UserTenantMapping {
        id: 0,
        center_user_id: center_user.id,
        customer_id: customer_id.clone(),
        customer_user_id: user.id,
        db_name: Some("public_magic".into()),
        created_at: ctx.timestamp,
        updated_at: None,
    });
    ctx.db.user_role().insert(UserRole {
        id: 0,
        user_id: user.id,
        role_id: role.role_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    insert_default_app_version(ctx, customer_id);
    insert_default_center_app_version(ctx);
    Ok(())
}
