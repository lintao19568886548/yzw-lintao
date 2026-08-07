//! 访客出入 Reducer 调用封装。

use crate::{services::spacetime::with_connection, spacetime_bindings::*};

use super::super::hr::finish_reducer;

pub async fn create_access_visitor_record(input: AccessVisitorInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_access_visitor_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增访客记录"));
            })
            .map_err(|error| format!("无法发送新增访客请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增访客回调已中断".to_string())?
}

pub async fn update_access_visitor_record(
    visitor_id: u64,
    input: AccessVisitorInput,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_access_visitor_then(visitor_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改访客记录"));
            })
            .map_err(|error| format!("无法发送修改访客请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改访客回调已中断".to_string())?
}

pub async fn delete_access_visitor_record(visitor_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_access_visitor_then(visitor_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除访客记录"));
            })
            .map_err(|error| format!("无法发送删除访客请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除访客回调已中断".to_string())?
}
