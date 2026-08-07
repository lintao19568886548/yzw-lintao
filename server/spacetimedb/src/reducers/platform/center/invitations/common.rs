//! 组织和租户邀请码共用的输入规范化与可用性校验。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Timestamp};

use crate::reducers::shared::validation::{normalize_optional_text, required_text, validate_max_length};

const MAX_INVITATION_USES: u32 = 500;

/// 邀请码必须由 Dioxus 客户端使用系统安全随机源生成后提交。
/// SpacetimeDB reducer 的确定性随机源不适合生成授权凭证。
#[derive(SpacetimeType)]
pub struct InvitationInput {
    pub code: String,
    pub role_ids: Vec<u64>,
    pub max_uses: Option<u32>,
    pub expires_at: Option<Timestamp>,
    pub remark: Option<String>,
}

#[pure_function::pure]
pub(super) fn normalize_code(code: String) -> Result<String, String> {
    let code = required_text(code, "邀请码不能为空")?
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if !(6..=32).contains(&code.len())
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err("邀请码必须由 6 到 32 位大写字母或数字组成".into());
    }
    Ok(code)
}

pub(super) fn validated_input(
    ctx: &ReducerContext,
    input: InvitationInput,
    allow_empty_roles: bool,
) -> Result<InvitationInput, String> {
    let code = normalize_code(input.code)?;
    let role_ids = input
        .role_ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if (!allow_empty_roles && role_ids.is_empty()) || role_ids.len() > 20 {
        return Err("邀请码必须配置 1 到 20 个角色".into());
    }
    if input
        .max_uses
        .is_some_and(|max_uses| max_uses == 0 || max_uses > MAX_INVITATION_USES)
    {
        return Err("邀请码最大使用次数必须在 1 到 500 之间".into());
    }
    if input
        .expires_at
        .is_some_and(|expires_at| expires_at <= ctx.timestamp)
    {
        return Err("邀请码过期时间必须晚于当前时间".into());
    }
    let remark = normalize_optional_text(input.remark);
    if let Some(remark) = remark.as_deref() {
        validate_max_length(remark, 200, "邀请码备注不能超过 200 个字符")?;
    }
    Ok(InvitationInput {
        code,
        role_ids,
        max_uses: input.max_uses,
        expires_at: input.expires_at,
        remark,
    })
}

pub(super) fn ensure_usable(
    status: &str,
    expires_at: Option<Timestamp>,
    max_uses: Option<u32>,
    used_count: u32,
    now: Timestamp,
) -> Result<(), String> {
    if status != "active" {
        return Err("邀请码已失效".into());
    }
    if expires_at.is_some_and(|expires_at| expires_at <= now) {
        return Err("邀请码已过期".into());
    }
    if max_uses.is_some_and(|max_uses| used_count >= max_uses) {
        return Err("邀请码使用次数已用完".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 邀请码会去除分隔符并转为大写() {
        assert_eq!(normalize_code(" abcd-1234 ".into()), Ok("ABCD1234".into()));
        assert!(normalize_code("错误码".into()).is_err());
    }

    #[test]
    fn 邀请码状态和使用次数会被校验() {
        assert!(ensure_usable("active", None, Some(2), 1, Timestamp::UNIX_EPOCH).is_ok());
        assert!(ensure_usable("active", None, Some(2), 2, Timestamp::UNIX_EPOCH).is_err());
        assert!(ensure_usable("revoked", None, None, 0, Timestamp::UNIX_EPOCH).is_err());
    }
}
