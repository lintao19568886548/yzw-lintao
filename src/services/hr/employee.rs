//! 员工档案 Reducer 调用封装。

use super::finish_reducer;
use crate::{services::spacetime::with_connection, spacetime_bindings::*};

/// 新增员工；`role_ids = None` 表示本次不修改账号角色。
pub async fn create_employee_record(
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_employee_with_roles_then(input, role_ids, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增员工"));
            })
            .map_err(|error| format!("无法发送新增员工请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增员工回调已中断".to_string())?
}

/// 更新员工，并在有角色管理权限时原子同步账号角色。
pub async fn update_employee_record(
    employee_id: u64,
    input: EmployeeInput,
    role_ids: Option<Vec<u64>>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_employee_with_roles_then(employee_id, input, role_ids, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改员工"));
            })
            .map_err(|error| format!("无法发送修改员工请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改员工回调已中断".to_string())?
}

pub async fn delete_employee_record(employee_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_employee_then(employee_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除员工"));
            })
            .map_err(|error| format!("无法发送删除员工请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除员工回调已中断".to_string())?
}
