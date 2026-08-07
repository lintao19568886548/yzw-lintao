//! 打卡点维护 Reducer 调用封装。

use super::finish_reducer;
use crate::{services::spacetime::with_connection, spacetime_bindings::*};

pub async fn create_attendance_location_record(
    input: AttendanceLocationInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_attendance_location_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增打卡点"));
            })
            .map_err(|error| format!("无法发送新增打卡点请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增打卡点回调已中断".to_string())?
}

pub async fn update_attendance_location_record(
    location_id: u64,
    input: AttendanceLocationInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_attendance_location_then(location_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "保存打卡点"));
            })
            .map_err(|error| format!("无法发送保存打卡点请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "保存打卡点回调已中断".to_string())?
}

pub async fn delete_attendance_location_record(location_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_attendance_location_then(location_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除打卡点"));
            })
            .map_err(|error| format!("无法发送删除打卡点请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除打卡点回调已中断".to_string())?
}
