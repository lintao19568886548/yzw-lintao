//! 合同主记录与图片关系 Reducer 调用封装。

use super::super::spacetime::with_connection;
use crate::spacetime_bindings::*;

fn finish_reducer<E: std::fmt::Display>(
    result: Result<Result<(), String>, E>,
    action: &'static str,
) -> Result<(), String> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(message),
        Err(error) => Err(format!("{action}失败：{error}")),
    }
}

pub async fn create_contract_with_images_record(
    input: RentalTenantInput,
    uploads: Vec<UploadedTenantImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_rental_tenant_with_images_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增合同"));
            })
            .map_err(|error| format!("无法发送新增合同请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增合同请求回调已中断".to_string())?
}

pub async fn update_contract_with_images_record(
    rental_tenant_id: u64,
    input: RentalTenantInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedTenantImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_rental_tenant_with_images_then(
                rental_tenant_id,
                input,
                existing_image_ids,
                uploads,
                move |_, result| {
                    let _ = sender.send(finish_reducer(result, "修改合同"));
                },
            )
            .map_err(|error| format!("无法发送修改合同请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改合同请求回调已中断".to_string())?
}

/// 逻辑删除合同主体；Module 同时解除图片关系并删除无引用图片元数据。
pub async fn delete_contract_record(rental_tenant_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_rental_tenant_then(rental_tenant_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除合同"));
            })
            .map_err(|error| format!("无法发送删除合同请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除合同请求回调已中断".to_string())?
}
