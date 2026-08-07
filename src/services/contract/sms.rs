//! 调用 SpacetimeDB Module Procedure 发送合同提醒短信。

#[derive(Clone, Debug, PartialEq)]
pub struct ContractSmsResult {
    pub rental_tenant_id: u64,
    pub success: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractSmsBatchResult {
    pub results: Vec<ContractSmsResult>,
}

pub async fn send_contract_reminder_sms(
    rental_tenant_ids: Vec<u64>,
    manual: bool,
) -> Result<ContractSmsBatchResult, String> {
    use super::super::spacetime::with_connection;
    use crate::spacetime_bindings::send_contract_reminder_sms_procedure::send_contract_reminder_sms;

    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection.procedures.send_contract_reminder_sms_then(
            rental_tenant_ids,
            manual,
            move |_, result| {
                let mapped = result
                    .map(|batch| ContractSmsBatchResult {
                        results: batch
                            .results
                            .into_iter()
                            .map(|item| ContractSmsResult {
                                rental_tenant_id: item.rental_tenant_id,
                                success: item.success,
                                message: item.message,
                            })
                            .collect(),
                    })
                    .map_err(|error| format!("合同提醒短信 Procedure 执行失败：{error}"));
                let _ = sender.send(mapped);
            },
        );
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "合同提醒短信请求回调已中断".to_string())?
}
