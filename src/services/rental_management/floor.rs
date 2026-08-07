//! 厂房楼层 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_factory_floor_record(
    factory_id: u64,
    input: FactoryFloorInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_factory_floor_then(factory_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增楼层"));
            })
            .map_err(|error| format!("无法发送新增楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增楼层回调已中断".to_string())?
}

/// 修改楼层主档并同步替换图片，服务端在同一事务内完成。
pub async fn update_factory_floor_with_images_record(
    floor_id: u64,
    input: FactoryFloorInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedFloorImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_factory_floor_with_images_then(
                floor_id,
                input,
                existing_image_ids,
                uploads,
                move |_, result| {
                    let _ = sender.send(finish_reducer(result, "修改楼层"));
                },
            )
            .map_err(|error| format!("无法发送修改楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改楼层回调已中断".to_string())?
}

pub async fn delete_factory_floor_record(floor_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_factory_floor_then(floor_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除楼层"));
            })
            .map_err(|error| format!("无法发送删除楼层请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除楼层回调已中断".to_string())?
}
