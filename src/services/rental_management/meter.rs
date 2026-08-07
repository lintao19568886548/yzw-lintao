//! 水电表台账 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_utility_meter_record(input: UtilityMeterInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_utility_meter_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增水电表"));
            })
            .map_err(|error| format!("无法发送新增水电表请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增水电表回调已中断".to_string())?
}

pub async fn update_utility_meter_record(
    meter_id: u64,
    input: UtilityMeterInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_utility_meter_then(meter_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改水电表"));
            })
            .map_err(|error| format!("无法发送修改水电表请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改水电表回调已中断".to_string())?
}

pub async fn delete_utility_meter_record(meter_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_utility_meter_then(meter_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除水电表"));
            })
            .map_err(|error| format!("无法发送删除水电表请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除水电表回调已中断".to_string())?
}
