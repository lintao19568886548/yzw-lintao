//! 验证独立 Rust 短信服务签发的 Access Token，并建立 SpacetimeDB 会话。

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use spacetimedb::{ReducerContext, Table};

use super::password::upsert_user_session;
use crate::{
    reducers::{
        access::{ADMIN_ROLE_NAME, AdminContext, SYSTEM_SCOPE},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

const LEGACY_ACCESS_TOKEN_KEY: &str = "legacy-access-token";

fn claim_text(claims: &Value, key: &str) -> Option<String> {
    claims
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn verify_access_token(token: &str, secret: &str, now_seconds: i64) -> Result<Value, String> {
    let parts = token.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err("短信登录凭证格式无效".into());
    }
    let header = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|_| "短信登录凭证格式无效")?;
    let header: Value = serde_json::from_slice(&header).map_err(|_| "短信登录凭证格式无效")?;
    if header.get("alg").and_then(Value::as_str) != Some("HS256") {
        return Err("短信登录凭证签名算法无效".into());
    }

    let signature = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| "短信登录凭证签名无效")?;
    let mut verifier =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| "短信登录密钥配置无效")?;
    verifier.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    verifier
        .verify_slice(&signature)
        .map_err(|_| "短信登录凭证签名无效")?;

    let payload = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "短信登录凭证格式无效")?;
    let claims: Value = serde_json::from_slice(&payload).map_err(|_| "短信登录凭证格式无效")?;
    let expires_at = claims
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or("短信登录凭证缺少有效期")?;
    if expires_at <= now_seconds {
        return Err("短信登录凭证已经过期".into());
    }
    if claims
        .get("nbf")
        .and_then(Value::as_i64)
        .is_some_and(|not_before| not_before > now_seconds)
    {
        return Err("短信登录凭证尚未生效".into());
    }
    Ok(claims)
}

fn ensure_customer(ctx: &ReducerContext, customer_id: &str) {
    if ctx
        .db
        .customer()
        .customer_id()
        .find(customer_id.to_string())
        .is_none()
    {
        ctx.db.customer().insert(Customer {
            customer_id: customer_id.into(),
            name: customer_id.into(),
            city: None,
            company_short_name: None,
            code: Some(customer_id.into()),
            status: 1,
            db_name: None,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
}

fn ensure_center_user(
    ctx: &ReducerContext,
    username: &str,
    real_name: &str,
    phone: Option<String>,
    customer_id: &str,
    home_path: Option<String>,
    token_version: u32,
) -> CenterUser {
    if let Some(mut user) = ctx.db.center_user().username().find(username.to_string()) {
        if user.phone.is_none() {
            user.phone = phone;
        }
        user.customer_type = Some(customer_id.into());
        user.home_path = home_path.or(user.home_path);
        user.token_version = token_version;
        user.updated_at = Some(ctx.timestamp);
        return ctx.db.center_user().id().update(user);
    }
    ctx.db.center_user().insert(CenterUser {
        id: 0,
        username: username.into(),
        real_name: real_name.into(),
        customer_type: Some(customer_id.into()),
        status: 1,
        token_version,
        phone,
        home_path,
        membership_trial_start_at: None,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

fn ensure_business_user(
    ctx: &ReducerContext,
    center_user: &CenterUser,
    customer_id: &str,
) -> Result<SystemUser, String> {
    if let Some(mapping) = ctx
        .db
        .user_tenant_mapping()
        .mapping_by_center_customer()
        .filter((center_user.id, customer_id))
        .next()
        && let Some(user) = ctx.db.system_user().id().find(mapping.customer_user_id)
    {
        return Ok(user);
    }
    if let Some(user) = ctx
        .db
        .system_user()
        .business_user_by_customer_username()
        .filter((customer_id, center_user.username.as_str()))
        .next()
    {
        if ctx
            .db
            .user_tenant_mapping()
            .mapping_by_customer_user()
            .filter((customer_id, user.id))
            .next()
            .is_some()
        {
            return Err("该租户账号已经绑定其他中心用户".into());
        }
        ctx.db.user_tenant_mapping().insert(UserTenantMapping {
            id: 0,
            center_user_id: center_user.id,
            customer_id: customer_id.into(),
            customer_user_id: user.id,
            db_name: None,
            created_at: ctx.timestamp,
            updated_at: None,
        });
        return Ok(user);
    }
    let business_user = ctx.db.system_user().insert(SystemUser {
        id: 0,
        username: center_user.username.clone(),
        customer_id: customer_id.into(),
        real_name: center_user.real_name.clone(),
        home_path: center_user.home_path.clone(),
        phone: center_user.phone.clone(),
        customer_type: Some(customer_id.into()),
        status: 1,
        token_version: center_user.token_version,
        created_at: ctx.timestamp,
        updated_at: None,
        avatar_url: None,
    });
    ctx.db.user_tenant_mapping().insert(UserTenantMapping {
        id: 0,
        center_user_id: center_user.id,
        customer_id: customer_id.into(),
        customer_user_id: business_user.id,
        db_name: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(business_user)
}

fn apply_token_roles(ctx: &ReducerContext, user_id: u64, customer_id: &str, claims: &Value) {
    let role_names = claims
        .get("roles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    for name in role_names {
        let role = ctx
            .db
            .role()
            .role_by_customer()
            .filter(customer_id)
            .find(|role| role.name == name)
            .unwrap_or_else(|| {
                ctx.db.role().insert(Role {
                    role_id: 0,
                    customer_id: customer_id.into(),
                    name: name.into(),
                    remark: Some("原短信登录凭证同步".into()),
                    status: 1,
                    rates: None,
                    parent_id: None,
                    reimbursement_auth: None,
                    organization_id: None,
                    scope: if name == ADMIN_ROLE_NAME {
                        SYSTEM_SCOPE.into()
                    } else {
                        "customer".into()
                    },
                    created_at: ctx.timestamp,
                    updated_at: None,
                })
            });
        if ctx
            .db
            .user_role()
            .user_role_by_pair()
            .filter((user_id, role.role_id))
            .next()
            .is_none()
        {
            ctx.db.user_role().insert(UserRole {
                id: 0,
                user_id,
                role_id: role.role_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
        }
    }
}

/// 为通过 Module 短信验证码校验的手机号创建或复用业务账号。
pub(crate) fn establish_phone_session(
    ctx: &ReducerContext,
    phone_number: &str,
    remember_me: bool,
) -> Result<(), String> {
    let customer_id = "public";
    ensure_customer(ctx, customer_id);
    let center_user = ensure_center_user(
        ctx,
        phone_number,
        phone_number,
        Some(phone_number.into()),
        customer_id,
        None,
        1,
    );
    if center_user.status != 1 {
        return Err("账号已被禁用".into());
    }
    let business_user = ensure_business_user(ctx, &center_user, customer_id)?;
    if business_user.status != 1 {
        return Err("当前租户账号已被禁用".into());
    }
    apply_token_roles(
        ctx,
        business_user.id,
        customer_id,
        &serde_json::json!({ "roles": ["User"] }),
    );
    upsert_user_session(ctx, center_user.id, remember_me);
    Ok(())
}

fn save_sms_auth_secret(ctx: &ReducerContext, secret: String) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let secret = required_text(secret, "原登录服务验证密钥不能为空")?;
    validate_max_length(&secret, 512, "原登录服务验证密钥不能超过 512 个字符")?;
    let key = LEGACY_ACCESS_TOKEN_KEY.to_string();
    let row = LegacyAuthConfig {
        config_key: key.clone(),
        access_token_secret: secret,
        updated_at: ctx.timestamp,
    };
    if ctx
        .db
        .legacy_auth_config()
        .config_key()
        .find(&key)
        .is_some()
    {
        ctx.db.legacy_auth_config().config_key().update(row);
    } else {
        ctx.db.legacy_auth_config().insert(row);
    }
    Ok(())
}

fn establish_sms_session(
    ctx: &ReducerContext,
    access_token: String,
    remember_me: bool,
) -> Result<(), String> {
    validate_max_length(&access_token, 16_384, "短信登录凭证过长")?;
    let config = ctx
        .db
        .legacy_auth_config()
        .config_key()
        .find(LEGACY_ACCESS_TOKEN_KEY.to_string())
        .ok_or("短信登录尚未配置")?;
    let now_seconds = ctx.timestamp.to_micros_since_unix_epoch() / 1_000_000;
    let claims = verify_access_token(&access_token, &config.access_token_secret, now_seconds)?;
    let username = claim_text(&claims, "username").ok_or("短信登录凭证缺少账号")?;
    let real_name = claim_text(&claims, "realName").unwrap_or_else(|| username.clone());
    let phone = claim_text(&claims, "phone").or_else(|| {
        (username.len() == 11 && username.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| username.clone())
    });
    let customer_id = claim_text(&claims, "customerId").unwrap_or_else(|| "public".into());
    validate_max_length(&username, 100, "短信登录账号过长")?;
    validate_max_length(&real_name, 100, "短信登录姓名过长")?;
    validate_max_length(&customer_id, 100, "短信登录租户标识过长")?;
    let home_path = claim_text(&claims, "homePath");
    let token_version = claims
        .get("tokenVersion")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1);

    ensure_customer(ctx, &customer_id);
    let center_user = ensure_center_user(
        ctx,
        &username,
        &real_name,
        phone,
        &customer_id,
        home_path,
        token_version,
    );
    if center_user.status != 1 {
        return Err("账号已被禁用".into());
    }
    let business_user = ensure_business_user(ctx, &center_user, &customer_id)?;
    if business_user.status != 1 {
        return Err("当前租户账号已被禁用".into());
    }
    apply_token_roles(ctx, business_user.id, &customer_id, &claims);
    upsert_user_session(ctx, center_user.id, remember_me);
    Ok(())
}

/// 管理员设置独立 Rust 短信服务的 HS256 验证密钥。
#[spacetimedb::reducer]
pub fn upsert_sms_auth_secret(ctx: &ReducerContext, secret: String) -> Result<(), String> {
    save_sms_auth_secret(ctx, secret)
}

/// 验证 Rust 短信服务返回的短期凭证，并创建当前设备会话。
#[spacetimedb::reducer]
pub fn login_with_sms_access_token(
    ctx: &ReducerContext,
    access_token: String,
    remember_me: bool,
) -> Result<(), String> {
    establish_sms_session(ctx, access_token, remember_me)
}

/// 兼容迁移期间已经生成的旧客户端调用名称。
#[spacetimedb::reducer]
pub fn upsert_legacy_auth_secret(ctx: &ReducerContext, secret: String) -> Result<(), String> {
    save_sms_auth_secret(ctx, secret)
}

/// 兼容迁移期间已经生成的旧客户端调用名称。
#[spacetimedb::reducer]
pub fn login_with_legacy_access_token(
    ctx: &ReducerContext,
    access_token: String,
    remember_me: bool,
) -> Result<(), String> {
    establish_sms_session(ctx, access_token, remember_me)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 非法短信登录凭证会被拒绝() {
        assert!(verify_access_token("invalid", "secret", 0).is_err());
    }
}
