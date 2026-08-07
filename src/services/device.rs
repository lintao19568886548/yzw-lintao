//! 设备台账 Reducer 调用封装。
//!
//! 图片与主档在服务端同一事务内落库（与维护设备同一手法），因此是 `async`
//! 形态：先传 R2，再一次性提交。设计见 `docs/设备管理.md`。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::hr::finish_reducer;

/// 保存设备台账；是否新增由 `id` 决定。设备图片同事务提交。
pub async fn save_device_asset_record(
    id: Option<u64>,
    input: DeviceAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDeviceImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let callback = move |_: &ReducerEventContext, result| {
            let _ = sender.send(finish_reducer(result, "保存设备"));
        };
        match id {
            Some(id) => connection.reducers.update_device_asset_then(
                id,
                input,
                existing_image_ids,
                uploads,
                callback,
            ),
            None => connection
                .reducers
                .create_device_asset_then(input, uploads, callback),
        }
        .map_err(|error| format!("无法发送设备保存请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "设备保存回调已中断".to_string())?
}

/// 注销设备（软删除）：数据保留，只从台账里隐藏。
pub async fn delete_device_asset_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_device_asset_then(id, move |_: &ReducerEventContext, result| {
                let _ = sender.send(finish_reducer(result, "注销设备"));
            })
            .map_err(|error| format!("无法发送设备注销请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "设备注销回调已中断".to_string())?
}

/// 建档或改档边缘计算设备；是否新增由 `id` 决定。
pub async fn save_edge_gateway_record(
    id: Option<u64>,
    input: EdgeGatewayInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let callback = move |_: &ReducerEventContext, result| {
            let _ = sender.send(finish_reducer(result, "保存边缘计算设备"));
        };
        match id {
            Some(id) => connection.reducers.update_edge_gateway_then(id, input, callback),
            None => connection.reducers.create_edge_gateway_then(input, callback),
        }
        .map_err(|error| format!("无法发送边缘计算设备保存请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "边缘计算设备保存回调已中断".to_string())?
}

/// 重发注册码，并解除现有身份绑定——换机器、重装系统、或那台电脑不再可信时用。
pub async fn regenerate_edge_registration_code_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .regenerate_edge_registration_code_then(id, move |_: &ReducerEventContext, result| {
                let _ = sender.send(finish_reducer(result, "重发注册码"));
            })
            .map_err(|error| format!("无法发送重发注册码请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "重发注册码回调已中断".to_string())?
}

/// 注销边缘计算设备（软删除），同时解绑身份。
pub async fn delete_edge_gateway_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_edge_gateway_then(id, move |_: &ReducerEventContext, result| {
                let _ = sender.send(finish_reducer(result, "注销边缘计算设备"));
            })
            .map_err(|error| format!("无法发送边缘计算设备注销请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "边缘计算设备注销回调已中断".to_string())?
}
