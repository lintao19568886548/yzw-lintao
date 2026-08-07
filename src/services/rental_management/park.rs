//! 园区主档 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

/// 建档并写入园区图片，服务端在同一事务内完成。
pub async fn create_park_with_images_record(
    input: ParkInput,
    uploads: Vec<UploadedParkImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_park_with_images_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增园区"));
            })
            .map_err(|error| format!("无法发送新增园区请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增园区回调已中断".to_string())?
}

/// 修改园区主档并同步替换图片，服务端在同一事务内完成。
pub async fn update_park_with_images_record(
    park_id: u64,
    input: ParkInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedParkImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_park_with_images_then(
                park_id,
                input,
                existing_image_ids,
                uploads,
                move |_, result| {
                    let _ = sender.send(finish_reducer(result, "修改园区"));
                },
            )
            .map_err(|error| format!("无法发送修改园区请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改园区回调已中断".to_string())?
}

pub async fn delete_park_record(park_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_park_then(park_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除园区"));
            })
            .map_err(|error| format!("无法发送删除园区请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除园区回调已中断".to_string())?
}
