//! 个人中心 Reducer 调用封装。
//!
//! 只能改本人资料——服务端从登录会话解析身份，这里不传任何目标用户参数。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::hr::finish_reducer;

/// 修改本人显示姓名。
pub async fn update_my_name_record(real_name: String) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_my_name_then(real_name, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改姓名"));
            })
            .map_err(|error| format!("无法发送修改姓名请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改姓名回调已中断".to_string())?
}

/// 更换本人头像；图片先上传 R2，再一次提交。
pub async fn update_my_avatar_record(upload: UploadedAvatarInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_my_avatar_then(upload, move |_, result| {
                let _ = sender.send(finish_reducer(result, "更换头像"));
            })
            .map_err(|error| format!("无法发送更换头像请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "更换头像回调已中断".to_string())?
}

/// 修改本人登录密码。成功后服务端会注销本设备之外的全部会话。
pub async fn change_my_password_record(
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .change_my_password_then(old_password, new_password, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改密码"));
            })
            .map_err(|error| format!("无法发送修改密码请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改密码回调已中断".to_string())?
}
