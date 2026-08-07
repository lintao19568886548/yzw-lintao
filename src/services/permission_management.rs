//! 角色权限策略 Reducer 调用封装。

use crate::{
    services::{hr::finish_reducer, spacetime::with_connection},
    spacetime_bindings::{
        delete_role_reducer::delete_role, save_role_policy_reducer::save_role_policy,
        RolePolicyInput,
    },
};

/// 原子保存角色基本信息、菜单权限与园区数据范围。
pub async fn save_role_policy(input: RolePolicyInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .save_role_policy_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "保存角色权限"));
            })
            .map_err(|error| format!("无法发送保存角色权限请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "保存角色权限回调已中断".to_string())?
}

/// 删除角色；服务端会拒绝删除系统管理员或仍有下级的角色。
pub async fn delete_role_policy(role_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_role_then(role_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除角色"));
            })
            .map_err(|error| format!("无法发送删除角色请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除角色回调已中断".to_string())?
}
