//! 客户/租户空间管理。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::AdminContext, platform::system::app_version::insert_default_app_version,
        validation::required_text,
    },
    tables::*,
};

/// 创建一个中心租户记录；实际数据库资源开通由后续部署流程处理。
#[spacetimedb::reducer]
pub fn create_customer(
    ctx: &ReducerContext,
    customer_id: String,
    name: String,
    db_name: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = required_text(customer_id, "租户标识不能为空")?;
    let name = required_text(name, "租户名称不能为空")?;
    if ctx.db.customer().customer_id().find(&customer_id).is_some() {
        return Err("租户标识已存在".into());
    }
    ctx.db.customer().insert(Customer {
        customer_id: customer_id.clone(),
        name,
        city: None,
        company_short_name: None,
        code: None,
        status: 1,
        db_name,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    insert_default_app_version(ctx, customer_id);
    Ok(())
}
