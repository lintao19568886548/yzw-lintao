//! API 操作审计日志写入逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{current_user_id, require_user},
        validation::{normalize_optional_text, validate_max_length},
    },
    tables::*,
};

/// 客户端可提交的操作上下文；用户和时间始终由服务端确定。
#[derive(SpacetimeType)]
pub struct ApiLogInput {
    pub method: String,
    pub path: Option<String>,
    pub referer_path: String,
    pub item_name: Option<String>,
}

fn validate_method(method: String) -> Result<String, String> {
    let method = method.trim().to_ascii_uppercase();
    matches!(method.as_str(), "POST" | "PUT" | "DELETE")
        .then_some(method)
        .ok_or("审计日志只记录 POST、PUT 或 DELETE 操作".into())
}

fn current_enabled_user(ctx: &ReducerContext) -> Result<SystemUser, String> {
    let user_id = current_user_id(ctx).ok_or("当前身份未绑定用户")?;
    let user = require_user(ctx, user_id)?;
    (user.status == 1)
        .then_some(user)
        .ok_or("用户已被禁用".into())
}

#[spacetimedb::reducer]
pub fn record_api_log(ctx: &ReducerContext, input: ApiLogInput) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let method = validate_method(input.method)?;
    let path = normalize_optional_text(input.path);
    let referer_path = input.referer_path.trim().to_string();
    let item_name = normalize_optional_text(input.item_name);
    if let Some(path) = path.as_deref() {
        validate_max_length(path, 255, "请求路径不能超过 255 个字符")?;
    }
    validate_max_length(&referer_path, 255, "访问路径不能超过 255 个字符")?;
    if let Some(item_name) = item_name.as_deref() {
        validate_max_length(item_name, 50, "操作项名称不能超过 50 个字符")?;
    }
    ctx.db.api_log().insert(ApiLog {
        log_id: 0,
        customer_id: user.customer_id,
        method,
        path,
        referer_path,
        item_name,
        username: user.real_name,
        user_id: user.id,
        request_time: ctx.timestamp,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 审计方法会转为大写并限制范围() {
        assert_eq!(validate_method(" post ".into()), Ok("POST".into()));
        assert!(validate_method("GET".into()).is_err());
    }
}
