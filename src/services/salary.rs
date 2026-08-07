//! 工资管理客户端 Reducer 调用封装。

use super::spacetime::with_connection;
use crate::spacetime_bindings::*;

// SpacetimeDB SDK 回调不保证处于 Dioxus Scope 中，因此回调只投递结果；
// 页面任务 await 后再更新 Signal，避免运行时作用域缺失和重入借用。

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

/// 新增工资并在同一事务中登记、绑定已上传到 R2 的凭证。
pub async fn create_salary_with_images_record(
    input: SalaryInput,
    uploads: Vec<UploadedSalaryImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .create_salary_with_images_then(input, uploads, move |_, result| {
                let _ = sender.send(finish_reducer(result, "新增工资记录"));
            })
            .map_err(|error| format!("无法发送新增工资请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "新增工资请求回调已中断".to_string())?
}

/// 修改工资，并以提交的凭证列表替换原图片关系。
pub async fn update_salary_with_images_record(
    salary_id: u64,
    input: SalaryInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedSalaryImageInput>,
) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .update_salary_with_images_then(
                salary_id,
                input,
                existing_image_ids,
                uploads,
                move |_, result| {
                    let _ = sender.send(finish_reducer(result, "修改工资记录"));
                },
            )
            .map_err(|error| format!("无法发送修改工资请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "修改工资请求回调已中断".to_string())?
}

/// 物理删除工资记录；Module 会同步删除图片关系和无引用图片元数据。
pub async fn delete_salary_record(salary_id: u64) -> Result<(), String> {
    let receiver = with_connection(|connection| {
        let (sender, receiver) = futures_channel::oneshot::channel();
        connection
            .reducers
            .delete_salary_then(salary_id, move |_, result| {
                let _ = sender.send(finish_reducer(result, "删除工资记录"));
            })
            .map_err(|error| format!("无法发送删除工资请求：{error}"))?;
        Ok(receiver)
    })?;
    receiver
        .await
        .map_err(|_| "删除工资请求回调已中断".to_string())?
}
