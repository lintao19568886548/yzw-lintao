//! 中心用户刷新令牌的登记、轮换和撤销逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::{current_center_user_id, require_center_user},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 刷新令牌只接收不可逆哈希和会话元数据。
#[derive(SpacetimeType)]
pub struct RefreshTokenInput {
    pub jti: String,
    pub token_hash: String,
    pub expires_at: Timestamp,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

fn current_enabled_center_user(ctx: &ReducerContext) -> Result<CenterUser, String> {
    let user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let user = require_center_user(ctx, user_id)?;
    (user.status == 1)
        .then_some(user)
        .ok_or("中心用户已被禁用".into())
}

#[pure_function::pure]
fn normalize_input(input: RefreshTokenInput) -> Result<RefreshTokenInput, String> {
    let jti = required_text(input.jti, "令牌标识不能为空")?;
    let token_hash = required_text(input.token_hash, "令牌哈希不能为空")?;
    validate_max_length(&jti, 64, "令牌标识不能超过 64 个字符")?;
    if token_hash.len() != 64 || !token_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("令牌哈希必须是 64 位十六进制字符串".into());
    }
    let ip = normalize_optional_text(input.ip);
    let user_agent = normalize_optional_text(input.user_agent);
    if let Some(ip) = ip.as_deref() {
        validate_max_length(ip, 64, "IP 地址不能超过 64 个字符")?;
    }
    if let Some(user_agent) = user_agent.as_deref() {
        validate_max_length(user_agent, 255, "用户代理不能超过 255 个字符")?;
    }
    Ok(RefreshTokenInput {
        jti,
        token_hash: token_hash.to_ascii_lowercase(),
        expires_at: input.expires_at,
        ip,
        user_agent,
    })
}

fn require_owned_token(
    ctx: &ReducerContext,
    user_id: u64,
    jti: &str,
) -> Result<RefreshToken, String> {
    ctx.db
        .refresh_token()
        .refresh_token_by_jti()
        .filter(jti)
        .find(|row| row.user_id == user_id)
        .ok_or("刷新令牌不存在".into())
}

fn ensure_unique_jti(ctx: &ReducerContext, jti: &str) -> Result<(), String> {
    ctx.db
        .refresh_token()
        .refresh_token_by_jti()
        .filter(jti)
        .next()
        .is_none()
        .then_some(())
        .ok_or("令牌标识已经存在".into())
}

fn insert_token(ctx: &ReducerContext, user_id: u64, input: RefreshTokenInput) {
    ctx.db.refresh_token().insert(RefreshToken {
        id: 0,
        user_id,
        jti: input.jti,
        token_hash: input.token_hash,
        expires_at: input.expires_at,
        revoked_at: None,
        replaced_by_jti: None,
        ip: input.ip,
        user_agent: input.user_agent,
        created_at: ctx.timestamp,
        updated_at: None,
    });
}

#[spacetimedb::reducer]
pub fn register_refresh_token(
    ctx: &ReducerContext,
    input: RefreshTokenInput,
) -> Result<(), String> {
    let user = current_enabled_center_user(ctx)?;
    let input = normalize_input(input)?;
    if input.expires_at <= ctx.timestamp {
        return Err("刷新令牌过期时间必须晚于当前时间".into());
    }
    ensure_unique_jti(ctx, &input.jti)?;
    insert_token(ctx, user.id, input);
    Ok(())
}

#[spacetimedb::reducer]
pub fn rotate_refresh_token(
    ctx: &ReducerContext,
    current_jti: String,
    replacement: RefreshTokenInput,
) -> Result<(), String> {
    let user = current_enabled_center_user(ctx)?;
    let current_jti = required_text(current_jti, "当前令牌标识不能为空")?;
    let replacement = normalize_input(replacement)?;
    let mut current = require_owned_token(ctx, user.id, &current_jti)?;
    if current.revoked_at.is_some() || current.expires_at <= ctx.timestamp {
        return Err("当前刷新令牌已失效".into());
    }
    if replacement.expires_at <= ctx.timestamp {
        return Err("新刷新令牌过期时间必须晚于当前时间".into());
    }
    ensure_unique_jti(ctx, &replacement.jti)?;
    current.revoked_at = Some(ctx.timestamp);
    current.replaced_by_jti = Some(replacement.jti.clone());
    current.updated_at = Some(ctx.timestamp);
    insert_token(ctx, user.id, replacement);
    ctx.db.refresh_token().id().update(current);
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_refresh_token(ctx: &ReducerContext, jti: String) -> Result<(), String> {
    let user = current_enabled_center_user(ctx)?;
    let jti = required_text(jti, "令牌标识不能为空")?;
    let mut token = require_owned_token(ctx, user.id, &jti)?;
    if token.revoked_at.is_none() {
        token.revoked_at = Some(ctx.timestamp);
        token.updated_at = Some(ctx.timestamp);
        ctx.db.refresh_token().id().update(token);
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn revoke_all_refresh_tokens(ctx: &ReducerContext) -> Result<(), String> {
    let user = current_enabled_center_user(ctx)?;
    let tokens = ctx
        .db
        .refresh_token()
        .refresh_token_by_user()
        .filter(user.id)
        .filter(|token| token.revoked_at.is_none())
        .collect::<Vec<_>>();
    for mut token in tokens {
        token.revoked_at = Some(ctx.timestamp);
        token.updated_at = Some(ctx.timestamp);
        ctx.db.refresh_token().id().update(token);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 令牌哈希必须是固定长度十六进制() {
        let input = RefreshTokenInput {
            jti: "jti-1".into(),
            token_hash: "A".repeat(64),
            expires_at: Timestamp::from_micros_since_unix_epoch(i64::MAX),
            ip: None,
            user_agent: None,
        };
        assert_eq!(normalize_input(input).unwrap().token_hash, "a".repeat(64));
    }
}
