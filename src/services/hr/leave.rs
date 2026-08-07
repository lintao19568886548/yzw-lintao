//! 请假申请与审批 Reducer 调用封装。

use super::finish_reducer;
use crate::{services::spacetime::with_connection, spacetime_bindings::*};

pub async fn create_leave_record(input: LeaveApplicationInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_leave_application_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "提交请假"));
            })
            .map_err(|error| format!("无法发送请假申请：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "请假申请回调已中断".to_string())?
}

pub async fn update_leave_record(id: u64, input: LeaveApplicationInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_leave_application_then(id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改请假"));
            })
            .map_err(|error| format!("无法发送修改请假请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改请假回调已中断".to_string())?
}

pub async fn audit_leave_record(id: u64, status: i8, reply: Option<String>) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .audit_leave_application_then(id, status, reply, move |_, result| {
                let _ = sender.send(finish_reducer(result, "审批请假"));
            })
            .map_err(|error| format!("无法发送请假审批：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "请假审批回调已中断".to_string())?
}

pub async fn delete_leave_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_leave_application_then(id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除请假"));
            })
            .map_err(|error| format!("无法发送删除请假请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除请假回调已中断".to_string())?
}
