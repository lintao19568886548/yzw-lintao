//! 个人中心：本人资料自助维护。
//!
//! 只能改**自己**的行——身份来自登录会话，Reducer 不接受目标用户参数，
//! 结构上不存在替别人改名换头像的路径。姓名改的是 `SystemUser.real_name`，
//! 业务侧（巡检人快照、审批人展示等）引用的就是这个字段；历史记录里的
//! 姓名快照不受改名影响，这正是快照的意义。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{current_center_user_id, current_user_id},
        platform::media::image_reducer::ensure_image,
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 用户确认保存后已经上传到 R2 的头像。
#[derive(SpacetimeType)]
pub struct UploadedAvatarInput {
    pub img_url: String,
    pub hash: String,
}

/// 修改本人显示姓名。
#[spacetimedb::reducer]
pub fn update_my_name(ctx: &ReducerContext, real_name: String) -> Result<(), String> {
    let mut user = require_self(ctx)?;
    let real_name = required_text(real_name, "姓名不能为空")?;
    validate_max_length(&real_name, 50, "姓名不能超过50个字符")?;
    user.real_name = real_name;
    user.updated_at = Some(ctx.timestamp);
    ctx.db.system_user().id().update(user);
    Ok(())
}

/// 更换本人头像。图片已上传 R2，这里登记元数据并落到用户行。
///
/// 旧头像的 R2 对象与图片元数据暂不回收：一人一张小图，量级可忽略；
/// 回收需要在用户行上多存一个 `img_id`，等真成为问题再加。
#[spacetimedb::reducer]
pub fn update_my_avatar(ctx: &ReducerContext, upload: UploadedAvatarInput) -> Result<(), String> {
    let mut user = require_self(ctx)?;
    ensure_image(
        ctx,
        user.customer_id.clone(),
        upload.img_url.clone(),
        upload.hash,
    )?;
    user.avatar_url = Some(upload.img_url);
    user.updated_at = Some(ctx.timestamp);
    ctx.db.system_user().id().update(user);
    Ok(())
}

/// 修改本人登录密码。
///
/// 旧密码验证通过后按登录同一套 BCrypt 口径写入新散列，随后**注销本设备之外
/// 的全部会话**——改密码往往发生在怀疑泄露之后，旧会话不该继续有效；当前
/// 设备保持登录，不打断操作者。
#[spacetimedb::reducer]
pub fn change_my_password(
    ctx: &ReducerContext,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定用户")?;
    check_new_password(&old_password, &new_password)?;
    let mut credential = ctx
        .db
        .user_credential()
        .center_user_id()
        .find(center_user_id)
        .ok_or("当前账号未设置密码，请联系管理员")?;
    if !bcrypt::verify(&old_password, &credential.password_hash).unwrap_or(false) {
        return Err("旧密码不正确".into());
    }
    let hash = bcrypt::hash_with_salt(&new_password, bcrypt::DEFAULT_COST, password_salt(ctx))
        .map_err(|_| "密码加密失败，请重试")?
        .format_for_version(bcrypt::Version::TwoB);
    credential.password_hash = hash;
    credential.password_updated_at = ctx.timestamp;
    credential.updated_at = Some(ctx.timestamp);
    ctx.db
        .user_credential()
        .center_user_id()
        .update(credential);

    let sender = ctx.sender();
    let other_sessions = ctx
        .db
        .user_session()
        .iter()
        .filter(|session| session.center_user_id == center_user_id && session.identity != sender)
        .map(|session| session.identity)
        .collect::<Vec<_>>();
    for identity in other_sessions {
        ctx.db.user_session().identity().delete(identity);
    }
    Ok(())
}

/// BCrypt 的盐要求 16 字节**唯一**值，而非保密值。模块沙箱里没有系统随机源
/// （依赖选用 bcrypt-no-getrandom 正是为此），以事务时间戳 + 调用者身份拼装，
/// 同一微秒同一连接不会发出两次改密码事务，唯一性成立。
fn password_salt(ctx: &ReducerContext) -> [u8; 16] {
    let mut salt = [0u8; 16];
    salt[..8].copy_from_slice(&ctx.timestamp.to_micros_since_unix_epoch().to_le_bytes());
    let sender = ctx.sender().to_string();
    for (index, byte) in sender.as_bytes().iter().take(8).enumerate() {
        salt[8 + index] = *byte;
    }
    salt
}

/// 新密码规则：8–128 位，且不得与旧密码相同。
#[pure_function::pure]
pub(crate) fn check_new_password(old_password: &str, new_password: &str) -> Result<(), String> {
    let length = new_password.chars().count();
    if !(8..=128).contains(&length) {
        return Err("新密码长度需在 8 到 128 位之间".into());
    }
    if new_password == old_password {
        return Err("新密码不能与旧密码相同".into());
    }
    Ok(())
}

/// 解析当前登录的业务用户本人；未绑定或已禁用给出明确原因。
fn require_self(ctx: &ReducerContext) -> Result<SystemUser, String> {
    let user_id = current_user_id(ctx).ok_or("当前身份未绑定用户")?;
    let user = ctx
        .db
        .system_user()
        .id()
        .find(user_id)
        .ok_or("用户不存在")?;
    if user.status != 1 {
        return Err("用户已被禁用".into());
    }
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 新密码必须八到一百二十八位() {
        assert!(check_new_password("old-pass", "short").is_err());
        assert!(check_new_password("old-pass", &"a".repeat(129)).is_err());
        assert!(check_new_password("old-pass", "long-enough-1").is_ok());
    }

    #[test]
    fn 新密码不能与旧密码相同() {
        assert!(check_new_password("same-password", "same-password").is_err());
        assert!(check_new_password("old-password", "new-password").is_ok());
    }
}
