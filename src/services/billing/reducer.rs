//! 账单管理客户端 Reducer 调用封装。

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

/// 新增账单；Module 会在同一事务中创建关联财务流水。
pub async fn create_amount_bill_record(input: AmountBillInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_amount_bill_then(input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增账单"));
            })
            .map_err(|error| format!("无法发送新增账单请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增账单请求回调已中断".to_string())?
}

/// 修改账单；Module 会同步修改账单关联的财务流水。
pub async fn update_amount_bill_record(bill_id: u64, input: AmountBillInput) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_amount_bill_then(bill_id, input, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改账单"));
            })
            .map_err(|error| format!("无法发送修改账单请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改账单请求回调已中断".to_string())?
}

/// 物理删除账单、水电明细和催收日志，并归档关联财务流水。
/// 确认已收齐——唯一能闭环的确认动作。要求园区管理权限，服务端为权威。
pub async fn confirm_bill_collected_record(bill_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .confirm_bill_collected_then(bill_id, None, move |_, result| {
                let _ = sender.send(finish_reducer(result, "确认收齐"));
            })
            .map_err(|error| format!("无法发送确认收齐请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "确认收齐请求回调已中断".to_string())?
}

/// 确认差额——只表示知悉，不闭环，确认后进入二次催交。
pub async fn confirm_bill_shortfall_record(bill_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .confirm_bill_shortfall_then(bill_id, None, move |_, result| {
                let _ = sender.send(finish_reducer(result, "确认差额"));
            })
            .map_err(|error| format!("无法发送确认差额请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "确认差额请求回调已中断".to_string())?
}

pub async fn delete_amount_bill_record(bill_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_amount_bill_then(bill_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除账单"));
            })
            .map_err(|error| format!("无法发送删除账单请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除账单请求回调已中断".to_string())?
}

/// 结算：把园区里所有未确认收齐的账单算进一个新的结转清单。只登记不改金额。
pub async fn open_carryover_batch_record(park_id: u64, period_label: String) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .open_carryover_batch_then(park_id, period_label, move |_, result| {
                let _ = sender.send(finish_reducer(result, "结算本期"));
            })
            .map_err(|error| format!("无法发送结算请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver.await.map_err(|_| "结算请求回调已中断".to_string())?
}

/// 逐张改处置：结转 / 本次不结转 / 剔除并确认收齐。
pub async fn set_carryover_disposition_record(
    item_id: u64,
    disposition: String,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .set_carryover_disposition_then(item_id, disposition, move |_, result| {
                let _ = sender.send(finish_reducer(result, "修改处置"));
            })
            .map_err(|error| format!("无法发送处置修改请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver.await.map_err(|_| "处置修改回调已中断".to_string())?
}

/// 确认整批结转。
pub async fn confirm_carryover_batch_record(batch_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .confirm_carryover_batch_then(batch_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "确认结转"));
            })
            .map_err(|error| format!("无法发送结转确认请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver.await.map_err(|_| "结转确认回调已中断".to_string())?
}

/// 丢弃一个待确认批次（算错了、想换账期标签重来）。
pub async fn discard_carryover_batch_record(batch_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .discard_carryover_batch_then(batch_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "丢弃结转清单"));
            })
            .map_err(|error| format!("无法发送丢弃请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver.await.map_err(|_| "丢弃请求回调已中断".to_string())?
}
