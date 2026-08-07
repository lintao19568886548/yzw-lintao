//! 租户系统配置维护逻辑。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
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

fn require_system_key(ctx: &ReducerContext, key_id: u64) -> Result<SystemKey, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .system_key()
        .key_id()
        .find(key_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("系统配置不存在".into())
}

#[spacetimedb::reducer]
pub fn upsert_system_key(
    ctx: &ReducerContext,
    key_name: String,
    value_json: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let key_name = required_text(key_name, "配置键不能为空")?;
    validate_max_length(&key_name, 100, "配置键不能超过 100 个字符")?;
    let value_json = normalize_json(value_json)?;
    if let Some(mut row) = ctx
        .db
        .system_key()
        .system_key_by_customer_name()
        .filter((customer_id.as_str(), key_name.as_str()))
        .next()
    {
        row.value_json = value_json;
        row.updated_at = Some(ctx.timestamp);
        ctx.db.system_key().key_id().update(row);
    } else {
        ctx.db.system_key().insert(SystemKey {
            key_id: 0,
            customer_id,
            key_name,
            value_json,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_system_key(ctx: &ReducerContext, key_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_system_key(ctx, key_id)?;
    ctx.db.system_key().key_id().delete(key_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 配置值会校验并规范化_json() {
        assert_eq!(
            normalize_json(" { \"b\": 2, \"a\": 1 } ".into()).unwrap(),
            "{\"a\":1,\"b\":2}"
        );
        assert!(normalize_json("not-json".into()).is_err());
    }
}
