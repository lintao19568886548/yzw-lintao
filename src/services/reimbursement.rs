//! 报销申请与审核 Reducer 的异步调用封装。

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

/// 创建报销申请，并在同一事务中登记、绑定刚上传的 R2 凭证。
pub async fn create_reimbursement_with_images_record(
    input: ReimbursementInput,
    uploads: Vec<UploadedReimbursementImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_reimbursement_with_images_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "提交报销申请"));
            })
            .map_err(|error| format!("无法发送报销申请：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "报销申请回调已中断".to_string())?
}

/// 审核人提交通过或驳回结果，服务端负责权限和金额上限校验。
pub async fn audit_reimbursement_record(
    id: u64,
    status: i8,
    opinion: Option<String>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .audit_reimbursement_then(id, status, opinion, move |_, result| {
                let _ = sender.send(finish_reducer(result, "审核报销申请"));
            })
            .map_err(|error| format!("无法发送审核请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "审核请求回调已中断".to_string())?
}

/// 撤销或删除报销；Module 同步归档财务记录并解除图片关系。
pub async fn delete_reimbursement_record(id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_reimbursement_then(id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除报销申请"));
            })
            .map_err(|error| format!("无法发送删除请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除请求回调已中断".to_string())?
}
