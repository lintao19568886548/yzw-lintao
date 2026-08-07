//! 厂房 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

/// 在同一个 Reducer 事务中创建厂房与楼层，避免产生没有父记录的孤立楼层。
pub async fn create_factory_with_floors_record(
    input: FactoryInput,
    floors: Vec<FactoryFloorInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_factory_with_floors_then(input, floors, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增厂房及楼层"));
            })
            .map_err(|error| format!("无法发送新增厂房及楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增厂房及楼层回调已中断".to_string())?
}

pub async fn update_factory_record(factory_id: u64, input: FactoryInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_factory_then(factory_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改厂房"));
            })
            .map_err(|error| format!("无法发送修改厂房请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改厂房回调已中断".to_string())?
}

pub async fn delete_factory_record(factory_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_factory_then(factory_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除厂房"));
            })
            .map_err(|error| format!("无法发送删除厂房请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除厂房回调已中断".to_string())?
}
