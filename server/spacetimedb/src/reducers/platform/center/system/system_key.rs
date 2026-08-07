//! 中心系统配置维护逻辑。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::AdminContext,
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[pure_function::pure]
fn normalize_json(value_json: String) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(value_json.trim()).map_err(|_| "配置值必须是有效 JSON")?;
    serde_json::to_string(&value).map_err(|_| "配置值无法序列化".into())
}

#[spacetimedb::reducer]
pub fn upsert_center_system_key(
    ctx: &ReducerContext,
    key_name: String,
    value_json: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let key_name = required_text(key_name, "配置键不能为空")?;
    validate_max_length(&key_name, 100, "配置键不能超过 100 个字符")?;
    let value_json = normalize_json(value_json)?;
    if let Some(mut row) = ctx
        .db
        .center_system_key()
        .center_system_key_by_name()
        .filter(key_name.as_str())
        .next()
    {
        row.value_json = value_json;
        row.updated_at = Some(ctx.timestamp);
        ctx.db.center_system_key().key_id().update(row);
    } else {
        ctx.db.center_system_key().insert(CenterSystemKey {
            key_id: 0,
            center_scope: 0,
            key_name,
            value_json,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_center_system_key(ctx: &ReducerContext, key_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    ctx.db
        .center_system_key()
        .key_id()
        .find(key_id)
        .ok_or("中心系统配置不存在")?;
    ctx.db.center_system_key().key_id().delete(key_id);
    Ok(())
}
