//! 独立租户主档 Reducer 的异步调用封装。

use super::spacetime::with_connection;
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

pub async fn create_tenant_profile_record(input: TenantProfileInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_tenant_profile_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增租户档案"));
            })
            .map_err(|error| format!("无法发送新增租户请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增租户请求回调已中断".to_string())?
}

pub async fn update_tenant_profile_record(
    tenant_profile_id: u64,
    input: TenantProfileInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_tenant_profile_then(tenant_profile_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改租户档案"));
            })
            .map_err(|error| format!("无法发送修改租户请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改租户请求回调已中断".to_string())?
}

pub async fn delete_tenant_profile_record(tenant_profile_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_tenant_profile_then(tenant_profile_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "停用租户档案"));
            })
            .map_err(|error| format!("无法发送停用租户请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "停用租户请求回调已中断".to_string())?
}

pub async fn sync_tenant_profiles_record() -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .sync_tenant_profiles_from_contracts_then(move |_, result| {
                let _ = sender.send(finish_reducer(result, "同步合同主体"));
            })
            .map_err(|error| format!("无法发送合同主体同步请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "合同主体同步回调已中断".to_string())?
}
