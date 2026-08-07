//! 调用 SpacetimeDB Module Procedure 发送账单催收短信。

use super::types::{CollectionSmsBatchResult, CollectionSmsResult};

/// Module 发布并重新生成绑定后，此函数直接调用催收 Procedure。
pub async fn send_collection_sms(
    bill_ids: Vec<u64>,
    collection_type: String,
    due_date: String,
    overdue_days: u32,
) -> Result<CollectionSmsBatchResult, String> {
    use super::super::spacetime::with_connection;
    use crate::spacetime_bindings::send_collection_sms_procedure::send_collection_sms;

    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection.procedures.send_collection_sms_then(
            bill_ids,
            collection_type,
            due_date,
            overdue_days,
            move |_, result| {
                let mapped = result
                    .map(|batch| CollectionSmsBatchResult {
                        results: batch
                            .results
                            .into_iter()
                            .map(|item| CollectionSmsResult {
                                bill_id: item.bill_id,
                                success: item.success,
                                message: item.message,
                            })
                            .collect(),
                    })
                    .map_err(|error| format!("催收短信 Procedure 执行失败：{error}"));
                let _ = sender.send(mapped);
            },
        );
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "催收短信请求回调已中断".to_string())?
}
