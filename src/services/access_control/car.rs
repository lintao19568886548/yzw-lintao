//! 车辆出入 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_access_car_record(input: AccessCarInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_access_car_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增车辆记录"));
            })
            .map_err(|error| format!("无法发送新增车辆请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增车辆回调已中断".to_string())?
}

pub async fn update_access_car_record(car_id: u64, input: AccessCarInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_access_car_then(car_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改车辆记录"));
            })
            .map_err(|error| format!("无法发送修改车辆请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改车辆回调已中断".to_string())?
}

pub async fn delete_access_car_record(car_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_access_car_then(car_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除车辆记录"));
            })
            .map_err(|error| format!("无法发送删除车辆请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除车辆回调已中断".to_string())?
}
