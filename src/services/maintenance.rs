//! 变压器资产台账与巡检 Reducer 调用封装。
//!
//! 图片与主档在服务端同一事务内落库（与园区档案同一手法），因此这里是
//! `async` 形态：上传 R2 之后一次性提交。巡检记录只增不删，没有更新或
//! 删除入口。设计见 `docs/变压器台账与扫码巡检.md`。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::hr::finish_reducer;

/// 保存变压器资产；是否新增由 `id` 决定。设备图片同事务提交。
pub async fn save_transformer_asset_record(
    id: Option<u64>,
    input: TransformerAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let callback = move |_: &ReducerEventContext, result| {
            let _ = sender.send(finish_reducer(result, "保存变压器"));
        };
        match id {
            Some(id) => connection
                .reducers
                .update_transformer_asset_then(id, input, existing_image_ids, uploads, callback),
            None => connection
                .reducers
                .create_transformer_asset_then(input, uploads, callback),
        }
        .map_err(|error| format!("无法发送变压器保存请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "变压器保存回调已中断".to_string())?
}

/// 注销变压器资产（软删除），巡检历史随之从视图隐藏但数据保留。
pub async fn delete_transformer_asset_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_transformer_asset_then(id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "注销变压器"));
            })
            .map_err(|error| format!("无法发送变压器注销请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "变压器注销回调已中断".to_string())?
}

/// 保存消防设施资产；是否新增由 `id` 决定。设施图片同事务提交。
pub async fn save_firefighting_asset_record(
    id: Option<u64>,
    input: FirefightingAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let callback = move |_: &ReducerEventContext, result| {
            let _ = sender.send(finish_reducer(result, "保存消防设施"));
        };
        match id {
            Some(id) => connection
                .reducers
                .update_firefighting_asset_then(id, input, existing_image_ids, uploads, callback),
            None => connection
                .reducers
                .create_firefighting_asset_then(input, uploads, callback),
        }
        .map_err(|error| format!("无法发送消防设施保存请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "消防设施保存回调已中断".to_string())?
}

/// 注销消防设施（软删除），巡检历史随之从视图隐藏但数据保留。
pub async fn delete_firefighting_asset_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_firefighting_asset_then(id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "注销消防设施"));
            })
            .map_err(|error| format!("无法发送消防设施注销请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "消防设施注销回调已中断".to_string())?
}

/// 提交一次消防设施巡检，现场照片同事务写入。
pub async fn create_firefighting_inspection_record(
    input: FirefightingInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_firefighting_inspection_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "提交巡检"));
            })
            .map_err(|error| format!("无法发送巡检提交请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "巡检提交回调已中断".to_string())?
}

/// 保存电梯资产；是否新增由 `id` 决定。设备图片同事务提交。
pub async fn save_elevator_asset_record(
    id: Option<u64>,
    input: ElevatorAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let callback = move |_: &ReducerEventContext, result| {
            let _ = sender.send(finish_reducer(result, "保存电梯"));
        };
        match id {
            Some(id) => connection
                .reducers
                .update_elevator_asset_then(id, input, existing_image_ids, uploads, callback),
            None => connection
                .reducers
                .create_elevator_asset_then(input, uploads, callback),
        }
        .map_err(|error| format!("无法发送电梯保存请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "电梯保存回调已中断".to_string())?
}

/// 注销电梯资产（软删除），巡检历史随之从视图隐藏但数据保留。
pub async fn delete_elevator_asset_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_elevator_asset_then(id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "注销电梯"));
            })
            .map_err(|error| format!("无法发送电梯注销请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "电梯注销回调已中断".to_string())?
}

/// 提交一次电梯巡检，现场照片同事务写入。
pub async fn create_elevator_inspection_record(
    input: ElevatorInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_elevator_inspection_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "提交巡检"));
            })
            .map_err(|error| format!("无法发送巡检提交请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "巡检提交回调已中断".to_string())?
}

/// 提交一次巡检，现场照片同事务写入。
pub async fn create_transformer_inspection_record(
    input: TransformerInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_transformer_inspection_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "提交巡检"));
            })
            .map_err(|error| format!("无法发送巡检提交请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "巡检提交回调已中断".to_string())?
}
