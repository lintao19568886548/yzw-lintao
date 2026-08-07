//! 中心 API 操作审计日志写入逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{current_center_user_id, require_center_user},
        validation::{normalize_optional_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct CenterApiLogInput {
    pub method: String,
    pub path: Option<String>,
    pub referer_path: String,
    pub item_name: Option<String>,
}

fn validate_method(method: String) -> Result<String, String> {
    let method = method.trim().to_ascii_uppercase();
    matches!(method.as_str(), "POST" | "PUT" | "DELETE")
        .then_some(method)
        .ok_or("中心审计日志只记录 POST、PUT 或 DELETE 操作".into())
}

#[spacetimedb::reducer]
pub fn record_center_api_log(ctx: &ReducerContext, input: CenterApiLogInput) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let user = require_center_user(ctx, center_user_id)?;
    if user.status != 1 {
        return Err("中心用户已被禁用".into());
    }
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
    ctx.db.center_api_log().insert(CenterApiLog {
        log_id: 0,
        center_scope: 0,
        method,
        path,
        referer_path,
        item_name,
        username: user.real_name,
        center_user_id,
        request_time: ctx.timestamp,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}
