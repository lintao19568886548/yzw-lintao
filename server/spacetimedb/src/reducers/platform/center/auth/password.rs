//! 账号密码校验和设备会话管理。

use spacetimedb::{ReducerContext, ScheduleAt, Table, TimeDuration};

use crate::{
    reducers::{
        access::{AdminContext, current_center_user_id, require_center_user},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

const SHORT_SESSION_MICROS: i64 = 12 * 60 * 60 * 1_000_000;
const REMEMBERED_SESSION_MICROS: i64 = 30 * 24 * 60 * 60 * 1_000_000;
/// 过期会话兜底清扫周期。View 无法过滤时间，过期会话最多多存活这么久。
const SESSION_SWEEP_INTERVAL_MICROS: i64 = 5 * 60 * 1_000_000;

/// 保证周期清扫任务存在。首次发布由 `init` 调用，之后每次登录再确认一次，
/// 使旧数据库在升级后也能自动补齐这个任务。
pub(crate) fn ensure_session_sweeper(ctx: &ReducerContext) {
    if ctx.db.user_session_sweep().count() != 0 {
        return;
    }
    ctx.db.user_session_sweep().insert(UserSessionSweep {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Interval(TimeDuration::from_micros(
            SESSION_SWEEP_INTERVAL_MICROS,
        )),
    });
}

/// 为当前设备创建或续期业务会话，供密码和短信登录共同使用。
pub(crate) fn upsert_user_session(ctx: &ReducerContext, center_user_id: u64, remember_me: bool) {
    let duration = if remember_me {
        REMEMBERED_SESSION_MICROS
    } else {
        SHORT_SESSION_MICROS
    };
    let identity = ctx.sender();
    let expires_at = ctx.timestamp + TimeDuration::from_micros(duration);
    if let Some(mut session) = ctx.db.user_session().identity().find(identity) {
        session.center_user_id = center_user_id;
        session.expires_at = expires_at;
        session.updated_at = Some(ctx.timestamp);
        ctx.db.user_session().identity().update(session);
    } else {
        ctx.db.user_session().insert(UserSession {
            identity,
            center_user_id,
            expires_at,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    ensure_session_sweeper(ctx);
    ctx.db.user_session_expiry().insert(UserSessionExpiry {
        scheduled_id: 0,
        scheduled_at: ScheduleAt::Time(expires_at),
        identity,
        expected_expires_at: expires_at,
    });
}

fn validate_password_hash(password_hash: String) -> Result<String, String> {
    let password_hash = required_text(password_hash, "密码哈希不能为空")?;
    validate_max_length(&password_hash, 100, "密码哈希不能超过 100 个字符")?;
    bcrypt::verify("", &password_hash)
        .map(|_| password_hash)
        .map_err(|_| "密码哈希不是有效的 BCrypt 格式".into())
}

fn find_login_user(ctx: &ReducerContext, account: &str) -> Option<CenterUser> {
    ctx.db
        .center_user()
        .username()
        .find(account.to_string())
        .or_else(|| {
            ctx.db
                .center_user()
                .iter()
                .find(|user| user.phone.as_deref() == Some(account))
        })
}

/// 校验账号密码并把当前 SpacetimeDB 设备身份登记为有效会话。
#[spacetimedb::reducer]
pub fn login_with_password(
    ctx: &ReducerContext,
    account: String,
    password: String,
    remember_me: bool,
) -> Result<(), String> {
    let account = required_text(account, "请输入账号")?;
    validate_max_length(&account, 100, "账号不能超过 100 个字符")?;
    if password.is_empty() || password.chars().count() > 128 {
        return Err("用户名或密码错误".into());
    }
    let user = find_login_user(ctx, &account)
        .filter(|user| user.status == 1)
        .ok_or("用户名或密码错误")?;
    let credential = ctx
        .db
        .user_credential()
        .center_user_id()
        .find(user.id)
        .ok_or("用户名或密码错误")?;
    if !bcrypt::verify(password, &credential.password_hash).unwrap_or(false) {
        return Err("用户名或密码错误".into());
    }

    upsert_user_session(ctx, user.id, remember_me);
    Ok(())
}

/// 调度器只删除仍与预期到期时间一致的会话，避免覆盖后续续期。
#[spacetimedb::reducer]
pub fn expire_user_session(ctx: &ReducerContext, task: UserSessionExpiry) -> Result<(), String> {
    if ctx.sender() != ctx.database_identity() {
        return Err("会话到期任务只允许调度器调用".into());
    }
    if let Some(session) = ctx.db.user_session().identity().find(task.identity)
        && session.expires_at == task.expected_expires_at
    {
        ctx.db.user_session().identity().delete(task.identity);
    }
    Ok(())
}

/// 周期清扫所有已过期的会话。
///
/// Reducer 侧已经按 `expires_at` 过滤，这里真正保护的是 View：View 拿不到
/// 时间，只能依赖过期行已被删除。一次性到期任务丢失时由这里补上。
#[spacetimedb::reducer]
pub fn sweep_expired_sessions(ctx: &ReducerContext, _task: UserSessionSweep) -> Result<(), String> {
    if ctx.sender() != ctx.database_identity() {
        return Err("会话清扫任务只允许调度器调用".into());
    }
    let expired = ctx
        .db
        .user_session()
        .user_session_by_expiry()
        .filter(..=ctx.timestamp)
        .map(|session| session.identity)
        .collect::<Vec<_>>();
    for identity in expired {
        ctx.db.user_session().identity().delete(identity);
    }
    Ok(())
}

/// 删除当前设备的业务登录会话，不删除 SpacetimeDB 设备身份令牌。
#[spacetimedb::reducer]
pub fn logout_current_session(ctx: &ReducerContext) -> Result<(), String> {
    ctx.db.user_session().identity().delete(ctx.sender());
    Ok(())
}

/// 管理员导入或更新中心账号的 BCrypt 密码哈希。
#[spacetimedb::reducer]
pub fn set_center_user_password_hash(
    ctx: &ReducerContext,
    center_user_id: u64,
    password_hash: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_center_user(ctx, center_user_id)?;
    let password_hash = validate_password_hash(password_hash)?;
    if let Some(mut credential) = ctx
        .db
        .user_credential()
        .center_user_id()
        .find(center_user_id)
    {
        credential.password_hash = password_hash;
        credential.password_updated_at = ctx.timestamp;
        credential.updated_at = Some(ctx.timestamp);
        ctx.db.user_credential().center_user_id().update(credential);
    } else {
        ctx.db.user_credential().insert(UserCredential {
            center_user_id,
            password_hash,
            password_updated_at: ctx.timestamp,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

/// 当前用户可以主动使自己的其他设备会话失效。
#[spacetimedb::reducer]
pub fn revoke_other_sessions(ctx: &ReducerContext) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份尚未登录")?;
    let current_identity = ctx.sender();
    let identities = ctx
        .db
        .user_session()
        .user_session_by_center_user()
        .filter(center_user_id)
        .filter(|session| session.identity != current_identity)
        .map(|session| session.identity)
        .collect::<Vec<_>>();
    for identity in identities {
        ctx.db.user_session().identity().delete(identity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use bcrypt::{Version, hash_with_salt};

    use super::*;

    #[test]
    fn bcrypt_凭据可以被确定性校验() {
        let hash = hash_with_salt("测试密码", 4, [7; 16])
            .unwrap()
            .format_for_version(Version::TwoB);
        assert_eq!(validate_password_hash(hash.clone()).unwrap(), hash);
        assert!(bcrypt::verify("测试密码", &hash).unwrap());
        assert!(!bcrypt::verify("错误密码", &hash).unwrap());
    }

    #[test]
    fn 非法密码哈希会被拒绝() {
        assert!(validate_password_hash("not-a-bcrypt-hash".into()).is_err());
    }
}
