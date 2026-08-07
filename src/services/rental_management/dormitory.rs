//! 宿舍 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_dormitory_record(input: DormitoryInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_dormitory_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增宿舍"));
            })
            .map_err(|error| format!("无法发送新增宿舍请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增宿舍回调已中断".to_string())?
}

/// 修改宿舍主档并同步替换图片，服务端在同一事务内完成。
pub async fn update_dormitory_with_images_record(
    dormitory_id: u64,
    input: DormitoryInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDormitoryImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_dormitory_with_images_then(
                dormitory_id,
                input,
                existing_image_ids,
                uploads,
                move |_, result| {
                    let _ = sender.send(finish_reducer(result, "修改宿舍"));
                },
            )
            .map_err(|error| format!("无法发送修改宿舍请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改宿舍回调已中断".to_string())?
}

pub async fn delete_dormitory_record(dormitory_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_dormitory_then(dormitory_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除宿舍"));
            })
            .map_err(|error| format!("无法发送删除宿舍请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除宿舍回调已中断".to_string())?
}
