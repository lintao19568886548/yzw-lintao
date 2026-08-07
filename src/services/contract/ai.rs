//! 合同图片多模态识别；密钥仅由 Dioxus 服务端读取。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractAiImage {
    /// 支持公开 HTTPS 图片地址或浏览器生成的 data URL。
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ContractAiDraft {
    pub tenant_name: String,
    pub phone_number: String,
    pub contract_start: String,
    pub contract_end: String,
    pub address: String,
    pub rent: String,
    pub area: String,
}

pub async fn analyze_contract_images(
    images: Vec<ContractAiImage>,
) -> Result<ContractAiDraft, String> {
    if images.is_empty() {
        return Err("请先选择至少一张合同图片".into());
    }
    let token =
        crate::services::credentials::load_saved_token().ok_or("登录凭证不存在，请重新登录")?;
    analyze_contract_images_server(token, images)
        .await
        .map_err(|error| format!("合同图片 AI 识别失败：{error}"))
}

#[post("/api/contract/images/analyze")]
async fn analyze_contract_images_server(
    token: String,
    images: Vec<ContractAiImage>,
) -> Result<ContractAiDraft, ServerFnError> {
    #[cfg(feature = "server")]
    {
        super::super::storage::validate_admin_access(&token).await?;
        if images.is_empty() || images.len() > 8 {
            return Err(ServerFnError::new("单次请选择 1 到 8 张合同图片"));
        }
        let total_length = images.iter().map(|image| image.source.len()).sum::<usize>();
        if total_length > 28 * 1024 * 1024 {
            return Err(ServerFnError::new("待识别图片总大小不能超过 20MB"));
        }
        if images.iter().any(|image| {
            !(image.source.starts_with("data:image/") || image.source.starts_with("https://"))
        }) {
            return Err(ServerFnError::new("合同图片地址格式无效"));
        }
        dotenvy::dotenv().ok();
        let key = std::env::var("ALIYUN_BAILIAN_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ServerFnError::new("服务端尚未配置 ALIYUN_BAILIAN_KEY"))?;
        let model =
            std::env::var("YIZU_CONTRACT_AI_MODEL").unwrap_or_else(|_| "qwen3.5-plus".into());
        let prompt = "请从合同图片中提取字段，只返回 JSON：{\"tenantName\":\"合同方姓名或企业名\",\"phoneNumber\":\"手机号，仅数字\",\"contractDate\":{\"start\":\"YYYY-MM-DD\",\"end\":\"YYYY-MM-DD\"},\"address\":\"租赁地点或地址\",\"rent\":\"月租金数字\",\"area\":\"出租面积数字\"}。缺失字段使用空字符串，不要猜测，不要添加说明。";
        let mut content = vec![serde_json::json!({ "type": "text", "text": prompt })];
        content.extend(images.into_iter().map(|image| {
            serde_json::json!({ "type": "image_url", "image_url": { "url": image.source } })
        }));
        let payload = serde_json::json!({
            "model": model,
            "temperature": 0,
            "max_tokens": 300,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "你负责提取合同字段，只返回规范 JSON。" },
                { "role": "user", "content": content }
            ]
        });
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| ServerFnError::new(format!("创建 AI 请求失败：{error}")))?
            .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
            .bearer_auth(key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|error| ServerFnError::new(format!("连接阿里云百炼失败：{error}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| ServerFnError::new(format!("读取 AI 响应失败：{error}")))?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/error/message")?
                        .as_str()
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(ServerFnError::new(format!("阿里云百炼拒绝请求：{message}")));
        }
        let value: Value = serde_json::from_str(&body)
            .map_err(|error| ServerFnError::new(format!("AI 响应格式无效：{error}")))?;
        let content = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| ServerFnError::new("AI 没有返回合同字段"))?;
        let parsed = parse_json_content(content)
            .ok_or_else(|| ServerFnError::new("AI 返回的合同 JSON 无法解析"))?;
        Ok(draft_from_json(&parsed))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, images);
        Err(ServerFnError::new("合同 AI 接口只能在服务端执行"))
    }
}

#[cfg(feature = "server")]
fn parse_json_content(content: &str) -> Option<Value> {
    serde_json::from_str(content.trim()).ok().or_else(|| {
        let start = content.find('{')?;
        let end = content.rfind('}')?;
        serde_json::from_str(&content[start..=end]).ok()
    })
}

#[cfg(feature = "server")]
fn value_text(value: Option<&Value>) -> String {
    value
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_f64().map(|number| number.to_string()))
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(feature = "server")]
fn draft_from_json(value: &Value) -> ContractAiDraft {
    let date = value.get("contractDate");
    ContractAiDraft {
        tenant_name: value_text(value.get("tenantName")),
        phone_number: value_text(value.get("phoneNumber"))
            .chars()
            .filter(char::is_ascii_digit)
            .collect(),
        contract_start: value_text(date.and_then(|date| date.get("start"))),
        contract_end: value_text(date.and_then(|date| date.get("end"))),
        address: value_text(value.get("address")),
        rent: value_text(value.get("rent")),
        area: value_text(value.get("area")),
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn 可以解析合同识别字段() {
        let value = serde_json::json!({
            "tenantName": "测试公司",
            "phoneNumber": "138-0013-8000",
            "contractDate": { "start": "2026-01-01", "end": "2026-12-31" },
            "rent": 12000,
            "area": "300.5"
        });
        let draft = draft_from_json(&value);
        assert_eq!(draft.phone_number, "13800138000");
        assert_eq!(draft.contract_start, "2026-01-01");
        assert_eq!(draft.rent, "12000");
    }
}
