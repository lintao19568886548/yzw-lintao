//! 财务流水 Reducer 的异步调用封装。

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

pub async fn create_finance_with_images_record(
    input: FinanceInput,
    image_urls: Vec<String>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_finance_with_images_then(input, image_urls, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增财务流水与凭证"));
            })
            .map_err(|error| format!("无法发送新增财务流水与凭证请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增财务流水与凭证回调已中断".to_string())?
}

pub async fn update_finance_with_images_record(
    finance_id: u64,
    input: FinanceInput,
    image_urls: Vec<String>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_finance_with_images_then(finance_id, input, image_urls, move |_, result| {
                let _ = sender.send(finish_reducer(result, "更新财务流水与凭证"));
            })
            .map_err(|error| format!("无法发送更新财务流水与凭证请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "更新财务流水与凭证回调已中断".to_string())?
}

pub async fn delete_finance_record(finance_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_finance_then(finance_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除财务流水"));
            })
            .map_err(|error| format!("无法发送删除财务流水请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除财务流水回调已中断".to_string())?
}
