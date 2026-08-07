//! 考勤打卡 Reducer 调用封装。

use super::finish_reducer;
use crate::{services::spacetime::with_connection, spacetime_bindings::*};

pub async fn punch_in_record(input: AttendancePunchInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .punch_in_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "上班打卡"));
            })
            .map_err(|error| format!("无法发送上班打卡请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "上班打卡回调已中断".to_string())?
}

pub async fn punch_out_record(
    attendance_id: u64,
    input: AttendancePunchInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .punch_out_then(attendance_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "下班打卡"));
            })
            .map_err(|error| format!("无法发送下班打卡请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "下班打卡回调已中断".to_string())?
}
