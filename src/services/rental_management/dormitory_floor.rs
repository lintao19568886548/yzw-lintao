//! 宿舍楼层 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_dormitory_floor_record(input: DormitoryFloorInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_dormitory_floor_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增宿舍楼层"));
            })
            .map_err(|error| format!("无法发送新增宿舍楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增宿舍楼层回调已中断".to_string())?
}

pub async fn update_dormitory_floor_record(
    dormitory_floor_id: u64,
    input: DormitoryFloorInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_dormitory_floor_then(dormitory_floor_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改宿舍楼层"));
            })
            .map_err(|error| format!("无法发送修改宿舍楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改宿舍楼层回调已中断".to_string())?
}

pub async fn delete_dormitory_floor_record(dormitory_floor_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_dormitory_floor_then(dormitory_floor_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除宿舍楼层"));
            })
            .map_err(|error| format!("无法发送删除宿舍楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除宿舍楼层回调已中断".to_string())?
}
